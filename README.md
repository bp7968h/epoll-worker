# epoll-worker 🚀

A high-performance, single-threaded event-driven server framework built from scratch in Rust. What started as a learning exercise for Linux epoll has evolved into a flexible, reusable server architecture demonstrating low-level systems programming and modern async patterns.

## Project Evolution

This project began as a simple broadcasting server to master Linux epoll programming, but has grown into a comprehensive server framework that can handle multiple use cases through a flexible event handler system. While still a work in progress, it demonstrates production-ready patterns used by high-performance servers like nginx and Redis.

## Learning Goals

This project was created as a learning exercise to master:

- **Linux epoll**: Event-driven I/O multiplexing for handling thousands of concurrent connections
- **FFI in Rust**: Safe interfacing with C system calls and kernel APIs  
- **Network Programming**: TCP socket management, non-blocking I/O, connection lifecycle
- **Systems Programming**: Understanding how modern web servers (nginx, Redis) work under the hood
- **Server Architecture**: Building flexible, reusable server components

## Architecture

### Core Components

- **epoll Event Loop**: Single-threaded event-driven architecture using Linux epoll
- **FFI Layer**: Direct system calls to `epoll_create`, `epoll_ctl`, `epoll_wait`, and `close`
- **Flexible Event Handlers**: Pluggable handler system for different server types
- **Connection Management**: Dynamic client registration/deregistration
- **Write Buffering**: Efficient queued writes with backpressure handling

### Key Features

- **High Concurrency**: Handles thousands of clients with minimal resource usage
- **Edge-Triggered Events**: Efficient event processing with `EPOLLET` flag
- **Graceful Shutdown**: Clean resource cleanup and connection termination
- **Non-blocking I/O**: Prevents server blocking on slow clients
- **Flexible Handlers**: Support for broadcast, echo, HTTP, and custom protocols
- **Production Patterns**: Proper error handling, logging, and resource management

## Quick Start

### Clone and Build

```bash
# Start the broadcast server
git clone git@github.com:bp7968h/epoll-worker.git && cd epoll-worker
cargo run
cargo build --release
```

### Running Example Servers

```bash
# Real-time chat/broadcast server
RUST_LOG=info cargo run --example broadcast_server

# Echo server (sends back what you type)
RUST_LOG=info cargo run --example echo_server

# Basic HTTP server
RUST_LOG=info cargo run --example http_server
```

### Connecting Clients

```bash
# Terminal 1: Start a client
RUST_LOG=info cargo run --example client 127.0.0.1:8080

# Terminal 2: Start another client  
RUST_LOG=info cargo run --example client 127.0.0.1:8080

# For HTTP server, use curl or browser
curl http://localhost:8080

# For HTTP load test
ab -n 100000 -c 1000 http://127.0.0.1:8080/
```

Now type messages in any client - they'll be broadcast to all other connected clients!

## Building Custom Servers

Create your own server by implementing the `EventHandler` trait

```rust
use epoll_worker::{EpollServer, EventHandler, HandlerAction};

struct MyHandler;

impl EventHandler for MyHandler {
    fn on_connection(&mut self, client_id: u64, stream: &TcpStream) -> std::io::Result<()> {
        // Handle new connections
        Ok(())
    }

    fn on_message(&mut self, client_id: u64, data: &[u8]) -> std::io::Result<HandlerAction> {
        // Process incoming messages
        Ok(HandlerAction::Reply(b"Hello!".to_vec()))
    }

    fn on_disconnect(&mut self, client_id: u64) -> std::io::Result<()> {
        // Clean up on disconnect
        Ok(())
    }

    fn is_data_complete(&mut self, data: &[u8]) -> bool {
        // Determine if message is complete
        true
    }
}

fn main() -> std::io::Result<()> {
    let handler = MyHandler;
    let mut server = EpollServer::new("127.0.0.1:8080", handler)?;
    server.run(None)
}
```

## Performance & Benchmarking

The benchmark/ directory contains comparison servers in Node.js and Python for performance testing. More optimization work is planned as the project continues to evolve.

## Technical Deep Dive

### epoll Fundamentals

This project demonstrates the **reactor pattern** used by high-performance servers:

1. **Register** file descriptors with epoll
2. **Wait** for events using `epoll_wait()`  
3. **Handle** ready connections without blocking
4. **Repeat** the cycle

```rust
// Core event loop (simplified)
while !shutdown {
    let events = epoll_wait(epfd, &mut events, timeout);
    for event in events {
        match event.role() {
            PeerRole::Server => accept_new_client(),
            PeerRole::Client(id) => handle_client_message(id),
        }
    }
}
```

### FFI Safety

All system calls are wrapped in `unsafe` blocks with proper error handling:

```rust
let result = unsafe { epoll_create(1) };
if result < 0 {
    return Err(Error::last_os_error());
}
```

### Memory Management

- **Zero-copy** where possible (direct buffer operations)
- **RAII** for automatic resource cleanup
- **Arc<AtomicBool>** for thread-safe shutdown signaling

### Connection Lifecycle

1. **Accept**: New connection triggers `EPOLLIN` on listener
2. **Register**: Add client socket to epoll interest list
3. **Communicate**: Handle `EPOLLIN` events for message reading
4. **Buffer Management**: Queue writes and handle backpressure
5. **Cleanup**: Remove from epoll and HashMap on `EPOLLRDHUP`

## Platform Support

`Linux Only` This project is specifically designed for Linux systems and uses Linux-specific epoll APIs. It will not run on other platforms.

## Current Status & Future Plans

This is a work in progress. As I continue learning systems programming, I plan to add:

- More comprehensive benchmarking
- Additional optimization techniques
- Extended protocol support
- Better error recovery mechanisms
- Performance monitoring tools

The codebase demonstrates production-ready patterns while remaining a learning vehicle for advanced systems programming concepts.

## 📖 References

- [The C10K Problem](http://www.kegel.com/c10k.html)
- [Linux epoll man page](https://man7.org/linux/man-pages/man7/epoll.7.html)
- [Rust FFI Documentation](https://doc.rust-lang.org/nomicon/ffi.html)
- [Event-driven architecture patterns](https://en.wikipedia.org/wiki/Event-driven_architecture)

---

**Built with Rust 🦀 | Learning Systems Programming | Understanding Modern Server Architecture**

*This project represents an ongoing journey in systems programming - expect continuous improvements and optimizations as new concepts are learned and applied.*