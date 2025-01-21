# networker-rs

`networker-rs` is a Rust library that provides networking utilities for TCP, UDP, and HTTP functionalities inspired by Go's `net` package. It simplifies common networking tasks such as dialing connections, listening for incoming connections, and sending/receiving HTTP requests.

## Features

- **TCP Support**
  - Dial connections to a specified address.
  - Listen for incoming connections.
  - Read and write to TCP streams.
  - Utility to read the latest message from a stream until the last newline.

- **UDP Support**
  - Send messages to a specified address.
  - Receive messages on a specified address.

- **HTTP Support**
  - Simple HTTP GET and POST request functionality.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
networker-rs = "0.1.0" # Replace with the latest version
```

## Example Usage

### TCP Example

```rust
use networker_rs::net;

fn main() {
    let address = "127.0.0.1:7878";

    // Start a TCP server in a separate thread
    std::thread::spawn(move || {
        let listener = net::listen(address).unwrap();
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            net::write_to_stream(&mut stream, b"Hello, client!").unwrap();
        }
    });

    // Connect to the server as a client
    let mut client = net::dial(address).unwrap();
    let mut buffer = [0; 512];
    let size = net::read_from_stream(&mut client, &mut buffer).unwrap();
    println!("Received: {}", String::from_utf8_lossy(&buffer[..size]));
}
```

### UDP Example

```rust
use networker_rs::net;

fn main() {
    let server_address = "127.0.0.1:8888";

    // Start a UDP listener in a separate thread
    std::thread::spawn(move || {
        let mut buffer = [0; 512];
        let (size, src) = net::udp_receive(server_address, &mut buffer).unwrap();
        println!("Received '{}' from {}", String::from_utf8_lossy(&buffer[..size]), src);
    });

    // Send a UDP message
    net::udp_send(server_address, b"Hello, UDP server!").unwrap();
}
```

### HTTP Example

```rust
use networker_rs::net::http;

fn main() {
    let address = "127.0.0.1:8080";

    // Simulate a basic HTTP server in a separate thread
    std::thread::spawn(move || {
        let listener = networker_rs::net::listen(address).unwrap();
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            networker_rs::net::write_to_stream(&mut stream, b"HTTP/1.1 200 OK

Content-Length: 13



Hello, world!").unwrap();
        }
    });

    // Perform an HTTP GET request
    let response = http::get(address, "/").unwrap();
    println!("Response: {}", response);
}
```

## License

