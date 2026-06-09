// SPDX-License-Identifier: MIT
use base64::{engine::general_purpose, Engine as _};
use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::{atomic::{AtomicU64, Ordering}, Arc, Mutex};
use std::thread;
use std::time::Duration;
use tungstenite::{accept, connect, Message};
use tungstenite::protocol::WebSocket;
use tungstenite::stream::MaybeTlsStream;
use url::Url;

pub mod net {
    use super::*;

    #[derive(Clone)]
    enum WsStream {
        Server(Arc<Mutex<WebSocket<TcpStream>>>),
        Client(Arc<Mutex<WebSocket<MaybeTlsStream<TcpStream>>>>),
    }

    const UDP_RELIABLE_PREFIX: &str = "__networker_reliable__";
    const UDP_ACK_PREFIX: &str = "__networker_ack__";
    const UDP_RELIABLE_HISTORY_LIMIT: usize = 256;
    static UDP_RELIABLE_COUNTER: AtomicU64 = AtomicU64::new(1);

    #[derive(Clone, Copy, Debug)]
    pub struct ReliabilityOptions {
        pub max_retries: usize,
        pub ack_timeout: Duration,
    }

    impl Default for ReliabilityOptions {
        fn default() -> Self {
            Self {
                max_retries: 20,
                ack_timeout: Duration::from_millis(50),
            }
        }
    }

    fn parse_udp_ack(message: &str) -> Option<u64> {
        let (prefix, id) = message.split_once(':')?;
        if prefix != UDP_ACK_PREFIX {
            return None;
        }
        id.parse().ok()
    }

    fn parse_udp_reliable(message: &str) -> Option<(u64, &str, &str)> {
        let (prefix, remainder) = message.split_once(':')?;
        if prefix != UDP_RELIABLE_PREFIX {
            return None;
        }
        let (id, remainder) = remainder.split_once(':')?;
        let (event, payload) = remainder.split_once(':')?;
        Some((id.parse().ok()?, event, payload))
    }

    fn dispatch_udp_event(socket: &Socket, event: &str, payload: &str) {
        if let Some(callback) = socket.handlers.lock().unwrap().get(event) {
            let payload_bytes = general_purpose::STANDARD
                .decode(payload)
                .unwrap_or_else(|_| payload.as_bytes().to_vec());
            callback(&payload_bytes);
        }
    }

    fn record_udp_reliable_packet(
        history: &Arc<Mutex<HashMap<SocketAddr, VecDeque<u64>>>>,
        peer: SocketAddr,
        packet_id: u64,
    ) -> bool {
        let mut history = history.lock().unwrap();
        let entry = history.entry(peer).or_insert_with(VecDeque::new);
        if entry.contains(&packet_id) {
            return false;
        }

        entry.push_back(packet_id);
        while entry.len() > UDP_RELIABLE_HISTORY_LIMIT {
            entry.pop_front();
        }

        true
    }

    pub struct EasySocketServer {
        handlers: Arc<Mutex<HashMap<String, Arc<dyn Fn(Socket) + Send + Sync + 'static>>>>,
        udp_reliable_history: Arc<Mutex<HashMap<SocketAddr, VecDeque<u64>>>>,
    }

    #[derive(Clone)]
    pub struct Socket {
        id: i32,
        stream: Option<Arc<Mutex<TcpStream>>>,
        udp_socket: Option<Arc<UdpSocket>>,
        udp_peer: Option<SocketAddr>,
        ws_stream: Option<WsStream>,
        handlers: Arc<Mutex<HashMap<String, Box<dyn Fn(&[u8]) + Send>>>>,
    }

    impl EasySocketServer {
        pub fn new() -> Self {
            Self {
                handlers: Arc::new(Mutex::new(HashMap::new())),
                udp_reliable_history: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        pub fn on<F>(&self, event: &str, callback: F)
        where
            F: Fn(Socket) + Send + Sync + 'static,
        {
            self.handlers.lock().unwrap().insert(event.to_string(), Arc::new(callback));
        }

        pub fn listen_tcp(&self, address: &str) -> io::Result<()> {
            let listener = TcpListener::bind(address)?;
            for stream in listener.incoming() {
                let stream = stream?;
                let socket = Socket::new_tcp(stream);
                let handlers = Arc::clone(&self.handlers);
                let callback = handlers.lock().unwrap().get("connection").cloned();
                if let Some(callback) = callback {
                    callback(socket);
                }
            }
            Ok(())
        }

        pub fn listen_udp(&self, address: &str) -> io::Result<()> {
            let socket = UdpSocket::bind(address)?;
            let udp_socket = Arc::new(socket);
            let mut buffer = [0; 1024];
            loop {
                if let Ok((size, src)) = udp_socket.recv_from(&mut buffer) {
                    let message = String::from_utf8_lossy(&buffer[..size]).to_string();
                    if let Some(ack_id) = parse_udp_ack(&message) {
                        println!("Received UDP ack: {}", ack_id);
                        continue;
                    }

                    if let Some((ack_id, event, payload)) = parse_udp_reliable(&message) {
                        let ack_message = format!("{UDP_ACK_PREFIX}:{ack_id}");
                        let _ = udp_socket.send_to(ack_message.as_bytes(), src);
                        if !record_udp_reliable_packet(&self.udp_reliable_history, src, ack_id) {
                            println!("Ignored duplicate reliable UDP packet: {}", ack_id);
                            continue;
                        }
                        let socket = Socket::new_udp_with_peer(udp_socket.clone(), src);
                        if let Some(callback) = self.handlers.lock().unwrap().get("connection").cloned() {
                            callback(socket.clone());
                        }
                        dispatch_udp_event(&socket, event, payload);
                    } else {
                        let (event, payload) = match message.split_once(':') {
                            Some((event, payload)) => (event, payload),
                            None => (message.as_str(), ""),
                        };
                        let socket = Socket::new_udp_with_peer(udp_socket.clone(), src);
                        if let Some(callback) = self.handlers.lock().unwrap().get("connection").cloned() {
                            callback(socket.clone());
                        }
                        dispatch_udp_event(&socket, event, payload);
                    }
                    println!("Received: {}", message);
                }
            }
        }

        pub fn listen_ws(&self, address: &str) -> io::Result<()> {
            let listener = TcpListener::bind(address)?;
            for stream in listener.incoming() {
                let stream = stream?;
                let websocket = accept(stream).map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;
                let socket = Socket::new_ws_server(websocket);
                let handlers = Arc::clone(&self.handlers);
                let callback = handlers.lock().unwrap().get("connection").cloned();
                if let Some(callback) = callback {
                    callback(socket);
                }
            }
            Ok(())
        }

        pub fn listen_tcp_background(self: Arc<Self>, address: String) {
            thread::spawn(move || {
                let _ = self.listen_tcp(&address);
            });
        }
    }

    impl Socket {
        pub fn new_tcp(stream: TcpStream) -> Self {
            Self {
                id: 0,
                stream: Some(Arc::new(Mutex::new(stream))),
                udp_socket: None,
                udp_peer: None,
                ws_stream: None,
                handlers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        pub fn new_udp(socket: Arc<UdpSocket>) -> Self {
            Self {
                id: 0,
                stream: None,
                udp_socket: Some(socket),
                udp_peer: None,
                ws_stream: None,
                handlers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn new_udp_with_peer(socket: Arc<UdpSocket>, peer: SocketAddr) -> Self {
            Self {
                id: 0,
                stream: None,
                udp_socket: Some(socket),
                udp_peer: Some(peer),
                ws_stream: None,
                handlers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn new_ws_server(websocket: WebSocket<TcpStream>) -> Self {
            Self {
                id: 0,
                stream: None,
                udp_socket: None,
                udp_peer: None,
                ws_stream: Some(WsStream::Server(Arc::new(Mutex::new(websocket)))),
                handlers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        pub fn ws(url: &str) -> tungstenite::Result<Self> {
            let (ws_stream, _) = connect(Url::parse(url).unwrap())?;
            Ok(Self {
                id: 0,
                stream: None,
                udp_socket: None,
                udp_peer: None,
                ws_stream: Some(WsStream::Client(Arc::new(Mutex::new(ws_stream)))),
                handlers: Arc::new(Mutex::new(HashMap::new())),
            })
        }

        pub fn id(&self) -> i32 {
            self.id
        }

        pub fn on<F>(&self, event: &str, callback: F)
        where
            F: Fn(&str) + Send + 'static,
        {
            self.on_bytes(event, move |payload| {
                let payload = String::from_utf8_lossy(payload);
                callback(payload.as_ref());
            });
        }

        pub fn on_bytes<F>(&self, event: &str, callback: F)
        where
            F: Fn(&[u8]) + Send + 'static,
        {
            self.handlers.lock().unwrap().insert(event.to_string(), Box::new(callback));
        }

        pub fn emit(&self, event: &str) {
            self.send(event, []);
        }

        pub fn emit_with(&self, event: &str, payload: &str) {
            self.send(event, payload.as_bytes());
        }

        pub fn send(&self, event: &str, payload: impl AsRef<[u8]>) {
            self.send_with_reliability(event, payload, false);
        }

        pub fn send_with_reliability(&self, event: &str, payload: impl AsRef<[u8]>, reliable: bool) {
            if reliable {
                let _ = self.send_reliable(event, payload);
                return;
            }

            self.send_unreliable(event, payload);
        }

        pub fn send_unreliable(&self, event: &str, payload: impl AsRef<[u8]>) {
            if let Some(stream) = &self.stream {
                let mut stream = stream.lock().unwrap();
                let encoded = general_purpose::STANDARD.encode(payload.as_ref());
                let _ = stream.write_all(format!("{event}:{encoded}").as_bytes());
                let _ = stream.write_all(b"\n");
                let _ = stream.flush();
                return;
            }

            if let (Some(udp_socket), Some(udp_peer)) = (&self.udp_socket, self.udp_peer) {
                let encoded = general_purpose::STANDARD.encode(payload.as_ref());
                let packet = format!("{event}:{encoded}");
                let _ = udp_socket.send_to(packet.as_bytes(), udp_peer);
                return;
            }

            if let Some(websocket) = &self.ws_stream {
                let encoded = general_purpose::STANDARD.encode(payload.as_ref());
                let message = Message::Text(format!("{event}:{encoded}"));
                match websocket {
                    WsStream::Server(websocket) => {
                        let mut websocket = websocket.lock().unwrap();
                        let _ = websocket.write_message(message);
                    }
                    WsStream::Client(websocket) => {
                        let mut websocket = websocket.lock().unwrap();
                        let _ = websocket.write_message(message);
                    }
                }
            }
        }

        pub fn send_reliable(&self, event: &str, payload: impl AsRef<[u8]>) -> io::Result<bool> {
            self.send_reliable_with_options(event, payload, ReliabilityOptions::default())
        }

        pub fn send_reliable_with_options(
            &self,
            event: &str,
            payload: impl AsRef<[u8]>,
            options: ReliabilityOptions,
        ) -> io::Result<bool> {
            if let (Some(udp_socket), Some(udp_peer)) = (&self.udp_socket, self.udp_peer) {
                let encoded = general_purpose::STANDARD.encode(payload.as_ref());
                let reliable_id = UDP_RELIABLE_COUNTER.fetch_add(1, Ordering::Relaxed);
                let packet = format!("{UDP_RELIABLE_PREFIX}:{reliable_id}:{event}:{encoded}");
                let ack_packet = format!("{UDP_ACK_PREFIX}:{reliable_id}");
                let receive_socket = udp_socket.try_clone()?;
                receive_socket.set_read_timeout(Some(options.ack_timeout))?;
                let mut buffer = [0; 1024];
                let max_attempts = options.max_retries.max(1);

                for _attempt in 0..max_attempts {
                    udp_socket.send_to(packet.as_bytes(), udp_peer)?;

                    loop {
                        match receive_socket.recv_from(&mut buffer) {
                            Ok((size, src)) if src == udp_peer => {
                                let received = String::from_utf8_lossy(&buffer[..size]).to_string();
                                if received == ack_packet {
                                    return Ok(true);
                                }
                            }
                            Ok(_) => {}
                            Err(err)
                                if err.kind() == io::ErrorKind::WouldBlock
                                    || err.kind() == io::ErrorKind::TimedOut =>
                            {
                                break;
                            }
                            Err(err) => return Err(err),
                        }
                    }
                }

                return Ok(false);
            }

            self.send_unreliable(event, payload);
            Ok(true)
        }

        pub fn listen_tcp(&self) {
            if let Some(stream) = &self.stream {
                let cloned = stream.lock().unwrap().try_clone();
                let Ok(stream) = cloned else {
                    return;
                };
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                loop {
                    line.clear();
                    let read = reader.read_line(&mut line);
                    let Ok(bytes) = read else {
                        break;
                    };
                    if bytes == 0 {
                        break;
                    }

                    let trimmed = line.trim_end_matches(['\r', '\n']);
                    let (event, payload) = match trimmed.split_once(':') {
                        Some((event, payload)) => (event, payload),
                        None => (trimmed, ""),
                    };

                    if let Some(callback) = self.handlers.lock().unwrap().get(event) {
                        let payload_bytes = general_purpose::STANDARD
                            .decode(payload)
                            .unwrap_or_else(|_| payload.as_bytes().to_vec());
                        callback(&payload_bytes);
                    }
                }
            }
        }

        pub fn listen_ws(&self) {
            if let Some(websocket) = &self.ws_stream {
                loop {
                    let message = match websocket {
                        WsStream::Server(websocket) => {
                            let mut websocket = websocket.lock().unwrap();
                            websocket.read_message()
                        }
                        WsStream::Client(websocket) => {
                            let mut websocket = websocket.lock().unwrap();
                            websocket.read_message()
                        }
                    };

                    let Ok(message) = message else {
                        break;
                    };

                    let Message::Text(text) = message else {
                        continue;
                    };

                    let (event, payload) = match text.split_once(':') {
                        Some((event, payload)) => (event, payload),
                        None => (text.as_str(), ""),
                    };

                    if let Some(callback) = self.handlers.lock().unwrap().get(event) {
                        let payload_bytes = general_purpose::STANDARD
                            .decode(payload)
                            .unwrap_or_else(|_| payload.as_bytes().to_vec());
                        callback(&payload_bytes);
                    }
                }
            }
        }

    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_tcp_server_client() {
            thread::spawn(|| {
                let server = EasySocketServer::new();
                server.on("connection", |socket| {
                    socket.on("hello", |msg| {
                        assert_eq!(msg, "world");
                    });
                    socket.on_bytes("bytes", |msg| {
                        assert_eq!(msg, b"raw");
                    });
                    socket.send("bytes", b"raw");
                    socket.listen_tcp();
                });
                server.listen_tcp("127.0.0.1:4000").unwrap();
            });
        }

        #[test]
        fn test_udp_send_reliable_repeats_datagram() {
            let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
            let receiver_addr = receiver.local_addr().unwrap();
            let sender = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
            let socket = Socket::new_udp_with_peer(sender, receiver_addr);

            let handle = thread::spawn(move || {
                let mut buffer = [0; 1024];
                let mut received = 0;

                loop {
                    let (size, src) = receiver.recv_from(&mut buffer).unwrap();
                    let message = String::from_utf8_lossy(&buffer[..size]).to_string();
                    if let Some((reliable_id, event, payload)) = parse_udp_reliable(&message) {
                        received += 1;
                        assert_eq!(event, "hello");
                        assert_eq!(payload, "d29ybGQ=");
                        if received >= 2 {
                            let ack = format!("{UDP_ACK_PREFIX}:{reliable_id}");
                            receiver.send_to(ack.as_bytes(), src).unwrap();
                            break;
                        }
                    }
                }

                received
            });

            socket.send_with_reliability("hello", b"world", true);

            let received = handle.join().unwrap();
            assert!(received >= 2);
        }

        #[test]
        fn test_udp_reliable_send_stops_after_retry_limit() {
            let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
            let receiver_addr = receiver.local_addr().unwrap();
            let sender = Arc::new(UdpSocket::bind("127.0.0.1:0").unwrap());
            let socket = Socket::new_udp_with_peer(sender, receiver_addr);

            let result = socket
                .send_reliable_with_options(
                    "hello",
                    b"world",
                    ReliabilityOptions {
                        max_retries: 2,
                        ack_timeout: Duration::from_millis(20),
                    },
                )
                .unwrap();

            assert!(!result);
        }

        #[test]
        fn test_udp_reliable_history_rejects_duplicates() {
            let history = Arc::new(Mutex::new(HashMap::new()));
            let peer: SocketAddr = "127.0.0.1:55555".parse().unwrap();

            assert!(record_udp_reliable_packet(&history, peer, 42));
            assert!(!record_udp_reliable_packet(&history, peer, 42));
        }

        #[test]
        fn test_ws_server_client() {
            use std::sync::mpsc;

            let (received_tx, received_rx) = mpsc::channel();

            thread::spawn(move || {
                let server = EasySocketServer::new();
                server.on("connection", move |socket| {
                    let received_tx = received_tx.clone();
                    socket.on("hello", move |msg| {
                        let _ = received_tx.send(msg.to_string());
                    });
                    socket.listen_ws();
                });
                server.listen_ws("127.0.0.1:4001").unwrap();
            });

            thread::sleep(Duration::from_millis(100));

            let client = Socket::ws("ws://127.0.0.1:4001").unwrap();
            client.send("hello", b"world");

            let received = received_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(received, "world");
        }
    }
}
