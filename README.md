
# networker-rs

`networker-rs` is a Rust library that provides networking utilities for TCP, UDP, WebSocket, and HTTP functionalities, inspired by Go's `net` package and JavaScript's `socket.io`. It simplifies common networking tasks and enables event-driven networking with an easy-to-use API.

Latest update: Made newline delimiters

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
use networker_rs::net::EasySocketServer;

fn main() {
    let server = EasySocketServer::new();
    server.on("connection", |socket| {
        socket.on("hello, server", |msg| {
            println!("Server received: {}", msg);
        });
        socket.emit("hello, client!");
        socket.listen_tcp();
    });
    server.listen_tcp("127.0.0.1:7878").unwrap();
}
```

### UDP Example

```rust
use networker_rs::net::EasySocketServer;

fn main() {
    let server = EasySocketServer::new();
    server.on("connection", |socket| {
        socket.on("hello, server", |msg| {
            println!("Server received: {}", msg);
        });
    });
    server.listen_udp("127.0.0.1:8888").unwrap();
}
```

### WebSocket Example

```rust
use networker_rs::net::EasySocketServer;

fn main() {
    let server = EasySocketServer::new();
    server.on("connection", |socket| {
        socket.on("hello, WebSocket server", |msg| {
            println!("Server received: {}", msg);
        });
    });
    server.listen_ws("127.0.0.1:9001").unwrap();
}
```

### HTTP Example

```rust
use networker_rs::net::EasySocketServer;

#[tokio::main]
async fn main() {
    let server = EasySocketServer::new();
    server.listen_http("127.0.0.1:8080").await.unwrap();
}
```

You can also get socket ids by doing either
```rust
// client side
fn client() {
    // ... bootstrapping code ...
    let sockID = client.id()
}
// server side
fn server() {
    // ... code up until sever.on
            let clientID = socket.id()
}
```

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

