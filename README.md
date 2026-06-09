
# networker-rs

`networker-rs` is a Rust library that provides networking utilities for TCP, UDP, and WebSocket functionality, inspired by Go's `net` package and JavaScript's `socket.io`. It simplifies common networking tasks and enables event-driven networking with an easy-to-use API.

Latest update: restored WebSocket support, added bounded reliable UDP retries,expanded the socket API for direct UDP connections

## Recent Changes

- Restored WebSocket client and server support.
- Added reliable UDP sends with retry and ACK handling.
- Added `Socket::udp(...)` for connecting directly to a UDP peer.
- Added `Socket::send_reliable_with_options(...)` for tuning retry count and ACK timeout.
- Reused UDP sockets per peer on the server side so repeated datagrams from the same address share one socket wrapper.

## Features

- **TCP Support**
  - Dial connections to a specified address.
  - Listen for incoming connections.
  - Emit events and handle specific message events.
  - Utility to read and write to TCP streams.

- **UDP Support**
  - Send messages to a specified address.
  - Optionally retry sends until the receiver acknowledges them.
  - Receive messages on a specified address.
  - Create a connected UDP socket with `Socket::udp(...)`.
  - Tune retry behavior with `ReliabilityOptions`.

- **WebSocket Support**
  - Connect to WebSocket servers.
  - Start a WebSocket server and handle bidirectional communication.
  - Emit and listen for events/messages.

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
        // retries delivery until acknowledged
        let _ = socket.send_reliable("hello, client", b"world");
    });
    server.listen_udp("127.0.0.1:8888").unwrap();
}
```

### Direct UDP Client Example

```rust
use networker_rs::net::Socket;

fn main() {
    let socket = Socket::udp("127.0.0.1:8888").unwrap();
    let _ = socket.send_reliable_with_options(
        "hello",
        b"world",
        networker_rs::net::ReliabilityOptions::default(),
    );
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

### WebSocket Client Example

```rust
use networker_rs::net::Socket;

fn main() {
    let socket = Socket::ws("ws://127.0.0.1:9001").unwrap();
    socket.send("hello", b"world");
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
