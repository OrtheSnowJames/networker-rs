use std::io::{self, BufRead, Read, Write, BufReader};
use std::net::{TcpListener, TcpStream, UdpSocket, ToSocketAddrs};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tungstenite::{accept, connect, Message};
use tungstenite::protocol::WebSocket;
use url::Url;

pub mod net {
    use super::*;

    pub struct EasySocket {
        tcp_stream: Option<TcpStream>,
        udp_socket: Option<UdpSocket>,
        ws_stream: Option<WebSocket<tungstenite::stream::MaybeTlsStream<TcpStream>>>,
        handlers: Arc<Mutex<HashMap<String, Box<dyn Fn(&str) + Send>>>>,
    }

    impl EasySocket {
        pub fn tcp(address: &str) -> io::Result<Self> {
            let tcp_stream = TcpStream::connect(address)?;
            Ok(Self {
                tcp_stream: Some(tcp_stream),
                udp_socket: None,
                ws_stream: None,
                handlers: Arc::new(Mutex::new(HashMap::new())),
            })
        }

        pub fn udp(address: &str) -> io::Result<Self> {
            let udp_socket = UdpSocket::bind(address)?;
            Ok(Self {
                tcp_stream: None,
                udp_socket: Some(udp_socket),
                ws_stream: None,
                handlers: Arc::new(Mutex::new(HashMap::new())),
            })
        }

        pub fn ws(url: &str) -> tungstenite::Result<Self> {
            let (ws_stream, _) = connect(url)?;
            Ok(Self {
                tcp_stream: None,
                udp_socket: None,
                ws_stream: Some(ws_stream),
                handlers: Arc::new(Mutex::new(HashMap::new())),
            })
        }

        pub fn emit(&mut self, event: &str) {
            if let Some(ref mut tcp) = self.tcp_stream {
                tcp.write_all(event.as_bytes()).unwrap();
            } else if let Some(ref mut ws) = self.ws_stream {
                ws.write_message(Message::Text(event.into())).unwrap();
            }
        }

        pub fn on<F>(&mut self, event: &str, callback: F)
        where
            F: Fn(&str) + Send + 'static,
        {
            self.handlers.lock().unwrap().insert(event.to_string(), Box::new(callback));
        }

        pub fn onmessage<F>(&mut self, callback: F)
        where
            F: Fn(&str) + Send + 'static,
        {
            self.on("message", callback);
        }

        pub fn listen(&mut self) {
            if let Some(ref mut tcp) = self.tcp_stream {
                let mut buffer = [0; 1024];
                let size = tcp.read(&mut buffer).unwrap();
                let message = String::from_utf8_lossy(&buffer[..size]).to_string();
                if let Some(callback) = self.handlers.lock().unwrap().get("message") {
                    callback(&message);
                }
                if let Some(callback) = self.handlers.lock().unwrap().get(&message) {
                    callback(&message);
                }
            } else if let Some(ref mut ws) = self.ws_stream {
                if let Ok(msg) = ws.read_message() {
                    if let Message::Text(text) = msg {
                        if let Some(callback) = self.handlers.lock().unwrap().get("message") {
                            callback(&text);
                        }
                        if let Some(callback) = self.handlers.lock().unwrap().get(text.as_str()) {
                            callback(&text);
                        }
                    }
                }
            }
        }
    }

    pub mod http {
        use super::*;

        pub struct EasyHttp;

        impl EasyHttp {
            pub fn get(address: &str, path: &str) -> io::Result<String> {
                let mut stream = TcpStream::connect(address)?;
                let request = format!("GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path);
                stream.write_all(request.as_bytes())?;

                let mut response = String::new();
                stream.read_to_string(&mut response)?;
                Ok(response)
            }

            pub fn post(address: &str, path: &str, body: &str) -> io::Result<String> {
                let mut stream = TcpStream::connect(address)?;
                let request = format!(
                    "POST {} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                    path,
                    body.len(),
                    body
                );
                stream.write_all(request.as_bytes())?;

                let mut response = String::new();
                stream.read_to_string(&mut response)?;
                Ok(response)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::net::{EasySocket, http::EasyHttp};
    use std::thread;
    use std::net::TcpListener;
    use std::io::Read;

    #[test]
    fn test_easy_socket() {
        thread::spawn(|| {
            let mut server = EasySocket::tcp("127.0.0.1:5767").unwrap();
            server.on("hello, server", |msg| {
                println!("Server received: {}", msg);
            });
            server.emit("hello, client!");
            server.listen();
        });

        thread::sleep(std::time::Duration::from_secs(1)); // Allow server to start

        let mut client = EasySocket::tcp("127.0.0.1:5767").unwrap();
        client.on("hello, client!", |msg| {
            println!("Client received: {}", msg);
        });
        client.emit("hello, server");
        client.listen();
    }

    #[test]
    fn test_easy_http() {
        use std::io::Write;
        thread::spawn(|| {
            let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buffer = [0; 512];
                stream.read(&mut buffer).unwrap();
                let response = "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, world!";
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        thread::sleep(std::time::Duration::from_secs(1)); // Allow server to start

        let response = EasyHttp::get("127.0.0.1:8080", "/").unwrap();
        assert!(response.contains("Hello, world!"));

        let post_response = EasyHttp::post("127.0.0.1:8080", "/submit", "{\"key\": \"value\"}").unwrap();
        println!("POST Response: {}", post_response);
    }
}
