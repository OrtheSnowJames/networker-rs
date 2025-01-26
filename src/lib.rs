use std::collections::HashMap;
use std::io::{self, BufRead, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use tungstenite::{accept, Message};
use hyper::{body::Body, Request, Response, Server, service::{make_service_fn, service_fn}};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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
        handlers: Arc<Mutex<HashMap<String, Box<dyn Fn(&str) + Send>>>>,
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
                if let Ok((size, src)) = udp_socket.recv_from(&mut buffer) {
                    let message = String::from_utf8_lossy(&buffer[..size]).to_string();
                    let handlers = Arc::clone(&self.handlers);
                    if let Some(callback) = handlers.lock().unwrap().get("connection") {
                        callback(Socket::new_udp(udp_socket.clone()));
                    }
                    println!("Received from {}: {}", src, message);
                }
            }
        }

        pub async fn listen_http(&self, address: &str) -> Result<(), Box<dyn std::error::Error>> {
            let make_svc = make_service_fn(|_conn| async {
                Ok::<_, hyper::Error>(service_fn(|_req: Request<Body>| async {
                    Ok::<_, hyper::Error>(Response::new(Body::from("Hello, HTTP!")))
                }))
            });
        
            let addr = address.parse()?; // Parse the address
            let server = Server::bind(&addr).serve(make_svc); // Use `try_bind` to bind to the address        
            println!("Listening on http://{}", address);
            server.await?;
            Ok(())
        }
        
        pub fn listen_ws(&self, address: &str) -> io::Result<()> {
            let listener = TcpListener::bind(address)?;
            for stream in listener.incoming() {
                let stream = stream?;
                let mut websocket = accept(stream).expect("Error during WebSocket handshake");
                if let Ok(Message::Text(msg)) = websocket.read_message() {
                    println!("WebSocket received: {}", msg);
                    websocket.write_message(Message::Text("Hello, WebSocket!".into())).unwrap();
                }
            }
            Ok(())
        }
    }

    impl Socket {
        fn generate_stable_id(addr: &str) -> i32 {
            let mut hasher = DefaultHasher::new();
            addr.hash(&mut hasher);
            (hasher.finish() & 0x7FFFFFFF) as i32 // Ensure positive i32
        }

        pub fn new_tcp(stream: TcpStream) -> Self {
            let addr = format!("{:?}", stream.peer_addr().unwrap_or_else(|_| panic!("Could not get peer address")));
            let id = Self::generate_stable_id(&addr);
            Self {
                id,
                stream: Some(Arc::new(Mutex::new(stream))),
                udp_socket: None,
                handlers: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        pub fn new_udp(socket: Arc<UdpSocket>) -> Self {
            let addr = format!("{:?}", socket.local_addr().unwrap_or_else(|_| panic!("Could not get local address")));
            let id = Self::generate_stable_id(&addr);
            Self {
                id,
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
            self.handlers.lock().unwrap().insert(event.to_string(), Box::new(callback));
        }

        pub fn emit(&self, event: &str) {
            if let Some(stream) = &self.stream {
                let mut stream = stream.lock().unwrap();
                let _ = stream.write_all(event.as_bytes());
            }
        }

        pub fn listen_tcp(&self) {
            let mut buffer = [0; 1024];
            if let Some(stream) = &self.stream {
                let mut stream = stream.lock().unwrap();
                if let Ok(size) = stream.read(&mut buffer) {
                    let message = String::from_utf8_lossy(&buffer[..size]).to_string();
                    if let Some(callback) = self.handlers.lock().unwrap().get(&message) {
                        callback(&message);
                    }
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_stable_socket_ids() {
            let addr = "127.0.0.1:8080";
            let id1 = Socket::generate_stable_id(addr);
            let id2 = Socket::generate_stable_id(addr);
            assert_eq!(id1, id2, "Same address should generate same ID");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::net::{self, EasySocketServer};
    use std::thread;

    #[test]
    fn test_tcp_server_client() {
        thread::spawn(|| {
            let server = EasySocketServer::new();
            server.on("connection", |socket| {
                socket.on("hello, server", |msg| {
                    println!("Server received: {}", msg);
                });
                socket.emit("hello, client!");
                socket.listen_tcp();
            });
            server.listen_tcp("127.0.0.1:4000").unwrap();
        });

        thread::sleep(std::time::Duration::from_secs(1)); // Allow server to start

        let client = std::net::TcpStream::connect("127.0.0.1:4000").unwrap();
        let socket = net::Socket::new_tcp(client);
        socket.on("hello, client!", |msg| {
            println!("Client received: {}", msg);
        });
        socket.emit("hello, server");
        socket.listen_tcp();
    }
}
