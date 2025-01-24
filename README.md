
# networker-rs

`networker-rs` is a Rust library that provides networking utilities for TCP, UDP, WebSocket, and HTTP functionalities, inspired by Go's `net` package and JavaScript's `socket.io`. It simplifies common networking tasks and enables event-driven networking with an easy-to-use API.

Latest update: Made the tcp calls with socket.emit and more easier calls, added ws with the same result.

## Features

- **TCP Support**
  - Dial connections to a specified address.
  - Listen for incoming connections.
  - Emit events and handle specific message events.
  - Utility to read and write to TCP streams.

- **UDP Support**
  - Send messages to a specified address.
  - Receive messages on a specified address.

- **WebSocket Support**
  - Connect to WebSocket servers.
  - Start a WebSocket server and handle bidirectional communication.
  - Emit and listen for events/messages.

- **HTTP Support**
  - Simple HTTP GET and POST request functionality.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
networker-rs = "0.2.0" # Replace with the latest version
```

## Example Usage

### TCP Example

```rust
use networker_rs::net::EasySocket;

fn main() {
    let mut socket = EasySocket::tcp("127.0.0.1:7878").unwrap();

    // Emit a message to the server
    socket.emit("hello, server");

    // Listen for generic messages
    socket.onmessage(|msg| {
        println!("Received message: {}", msg);
    });

    // Listen for a specific event
    socket.on("hello, client!", |msg| {
        println!("Received specific event: {}", msg);
    });

    // Start listening for messages
    socket.listen();
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

### WebSocket Example

```rust
use networker_rs::net::EasySocket;

fn main() {
    let mut socket = EasySocket::ws("ws://127.0.0.1:9001").unwrap();

    // Emit a message to the WebSocket server
    socket.emit("hello, WebSocket server");

    // Listen for WebSocket messages
    socket.onmessage(|msg| {
        println!("Received WebSocket message: {}", msg);
    });

    // Start listening for messages
    socket.listen();
}
```

### HTTP Example

```rust
use networker_rs::net::http::EasyHttp;

fn main() {
    let address = "127.0.0.1:8080";

    // Perform an HTTP GET request
    let response = EasyHttp::get(address, "/").unwrap();
    println!("GET Response: {}", response);

    // Perform an HTTP POST request
    let post_response = EasyHttp::post(address, "/submit", "{"key": "value"}").unwrap();
    println!("POST Response: {}", post_response);
}
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
