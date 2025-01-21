use std::io::{self, BufRead, Read, Write, BufReader};
use std::net::{TcpListener, TcpStream, UdpSocket, ToSocketAddrs};
use std::collections::VecDeque;

pub mod net {
    use super::*;

    /// Dial a TCP connection to the specified address.
    pub fn dial<A: ToSocketAddrs>(address: A) -> io::Result<TcpStream> {
        TcpStream::connect(address)
    }

    /// Start a TCP listener at the specified address.
    pub fn listen<A: ToSocketAddrs>(address: A) -> io::Result<TcpListener> {
        TcpListener::bind(address)
    }

    /// Send a UDP message to the specified address.
    pub fn udp_send<A: ToSocketAddrs>(address: A, message: &[u8]) -> io::Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.send_to(message, address)?;
        Ok(())
    }

    /// Receive a UDP message on the specified address.
    pub fn udp_receive<A: ToSocketAddrs>(address: A, buffer: &mut [u8]) -> io::Result<(usize, String)> {
        let socket = UdpSocket::bind(address)?;
        let (size, src) = socket.recv_from(buffer)?;
        Ok((size, src.to_string()))
    }

    /// Utility function to write data to a TCP stream.
    pub fn write_to_stream(stream: &mut TcpStream, data: &[u8]) -> io::Result<()> {
        stream.write_all(data)
    }

    /// Utility function to read data from a TCP stream.
    pub fn read_from_stream(stream: &mut TcpStream, buffer: &mut [u8]) -> io::Result<usize> {
        stream.read(buffer)
    }

    /// Read the latest message from the TCP stream until the last newline character.
    pub fn read_latest_message(stream: &mut TcpStream) -> io::Result<String> {
        let mut reader = BufReader::new(stream);
        let mut buffer = VecDeque::new();
        let mut line = String::new();

        while reader.read_line(&mut line)? > 0 {
            buffer.push_back(line.clone());
            line.clear();
        }

        Ok(buffer.pop_back().unwrap_or_default().trim().to_string())
    }

    pub mod http {
        use super::*;

        /// Make a simple HTTP GET request to the specified address.
        pub fn get<A: ToSocketAddrs>(address: A, path: &str) -> io::Result<String> {
            let mut stream = TcpStream::connect(address)?;
            let request = format!("GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path);
            stream.write_all(request.as_bytes())?;

            let mut response = String::new();
            stream.read_to_string(&mut response)?;
            Ok(response)
        }

        /// Make a simple HTTP POST request to the specified address with a body.
        pub fn post<A: ToSocketAddrs>(address: A, path: &str, body: &str) -> io::Result<String> {
            let mut stream = TcpStream::connect(address)?;
            let request = format!(
                "POST {} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
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

#[cfg(test)]
mod tests {
    use super::net;
    use std::thread;

    #[test]
    fn test_tcp_connection() {
        let address = "127.0.0.1:7878";

        // Start a server in a separate thread
        thread::spawn(move || {
            let listener = net::listen(address).unwrap();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buffer = [0; 512];
                net::read_from_stream(&mut stream, &mut buffer).unwrap();
                net::write_to_stream(&mut stream, b"Hello, client!").unwrap();
            }
        });

        // Simulate a client
        let mut client = net::dial(address).unwrap();
        net::write_to_stream(&mut client, b"Hello, server!").unwrap();

        let mut buffer = [0; 512];
        let size = net::read_from_stream(&mut client, &mut buffer).unwrap();
        assert_eq!(String::from_utf8_lossy(&buffer[..size]), "Hello, client!");
    }

    #[test]
    fn test_udp_message() {
        let server_address = "127.0.0.1:8888";
        let client_address = "127.0.0.1:0";

        let handle = thread::spawn(move || {
            let mut buffer = [0; 512];
            let (size, src) = net::udp_receive(server_address, &mut buffer).unwrap();
            assert_eq!(src, "127.0.0.1:0");
            assert_eq!(String::from_utf8_lossy(&buffer[..size]), "Hello, UDP server!");
        });

        net::udp_send(server_address, b"Hello, UDP server!").unwrap();
        handle.join().unwrap();
    }

    #[test]
    fn test_read_latest_message() {
        let address = "127.0.0.1:8989";

        thread::spawn(move || {
            let listener = net::listen(address).unwrap();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                net::write_to_stream(&mut stream, b"yes\nno\nye\n").unwrap();
            }
        });

        let mut client = net::dial(address).unwrap();
        let latest_message = net::read_latest_message(&mut client).unwrap();
        assert_eq!(latest_message, "ye");
    }

    #[test]
    fn test_http_get() {
        // Simulate a basic HTTP server
        let address = "127.0.0.1:8080";
        thread::spawn(move || {
            let listener = net::listen(address).unwrap();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buffer = [0; 512];
                net::read_from_stream(&mut stream, &mut buffer).unwrap();
                net::write_to_stream(&mut stream, b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, world!").unwrap();
            }
        });

        let response = net::http::get(address, "/").unwrap();
        assert!(response.contains("Hello, world!"));
    }

    #[test]
    fn test_http_post() {
        // Simulate a basic HTTP server
        let address = "127.0.0.1:8081";
        thread::spawn(move || {
            let listener = net::listen(address).unwrap();
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buffer = [0; 512];
                net::read_from_stream(&mut stream, &mut buffer).unwrap();
                net::write_to_stream(&mut stream, b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nSuccess").unwrap();
            }
        });

        let response = net::http::post(address, "/submit", "data").unwrap();
        assert!(response.contains("Success"));
    }
}

