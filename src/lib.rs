// SPDX-License-Identifier: MIT
use base64::{engine::general_purpose, Engine as _};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;

pub mod net {
    use super::*;

    pub struct EasySocketServer {
        handlers: Arc<Mutex<HashMap<String, Arc<dyn Fn(Socket) + Send + Sync + 'static>>>>,
    }

    #[derive(Clone)]
    pub struct Socket {
        id: i32,
        stream: Option<Arc<Mutex<TcpStream>>>,
        udp_socket: Option<Arc<UdpSocket>>,
        handlers: Arc<Mutex<HashMap<String, Box<dyn Fn(&[u8]) + Send>>>>,
    }

    impl EasySocketServer {
        pub fn new() -> Self {
            Self {
                handlers: Arc::new(Mutex::new(HashMap::new())),
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
                if let Ok((size, _src)) = udp_socket.recv_from(&mut buffer) {
                    let message = String::from_utf8_lossy(&buffer[..size]).to_string();
                    let handlers = Arc::clone(&self.handlers);
                    if let Some(callback) = handlers.lock().unwrap().get("connection") {
                        callback(Socket::new_udp(udp_socket.clone()));
                    }
                    println!("Received: {}", message);
                }
            }
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
                handlers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        pub fn new_udp(socket: Arc<UdpSocket>) -> Self {
            Self {
                id: 0,
                stream: None,
                udp_socket: Some(socket),
                handlers: Arc::new(Mutex::new(HashMap::new())),
            }
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
            if let Some(stream) = &self.stream {
                let mut stream = stream.lock().unwrap();
                let encoded = general_purpose::STANDARD.encode(payload.as_ref());
                let _ = stream.write_all(format!("{event}:{encoded}").as_bytes());
                let _ = stream.write_all(b"\n");
                let _ = stream.flush();
            }
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
    }
}
