# epoll-broadcaster 📡

A single threaded, multi client event driven broadcast server built from scratch in Rust, demonstrating low-level Linux epoll programming, FFI (Foreign Function Interface), and software engineering practices.

## Learning Goals

This project was created as a learning exercise to master:

- **Linux epoll**: Event-driven I/O multiplexing for handling thousands of concurrent connections
- **FFI in Rust**: Safe interfacing with C system calls and kernel APIs  
- **Network Programming**: TCP socket management, non-blocking I/O, connection lifecycle
- **Systems Programming**: Understanding how modern web servers (nginx, Redis) work under the hood
- **Production Engineering**: Testing strategies, error handling, and code organization

## Architecture

### Core Components

- **epoll Event Loop**: Single-threaded event-driven architecture using Linux epoll
- **FFI Layer**: Direct system calls to `epoll_create`, `epoll_ctl`, `epoll_wait`, and `close`
- **Connection Management**: Dynamic client registration/deregistration
- **Message Broadcasting**: Real-time message distribution to all connected clients

### Key Features

- **High Concurrency**: Handle multiple clients with minimal resource usage
- **Edge-Triggered Events**: Efficient event processing with `EPOLLET` flag
- **Graceful Shutdown**: Clean resource cleanup and connection termination
- **Non-blocking I/O**: Prevents server blocking on slow clients

## Quick Start

### Running the Server

```bash
# Start the broadcast server
git clone git@github.com:bp7968h/epoll-broadcaster.git && cd epoll-broadcaster
cargo run
```

### Connecting Clients

```bash
# Terminal 1: Start a client
cargo run --bin client

# Terminal 2: Start another client  
cargo run --bin client
```

Now type messages in any client - they'll be broadcast to all other connected clients!

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
4. **Cleanup**: Remove from epoll and HashMap on `EPOLLRDHUP`

## 📖 References

- [The C10K Problem](http://www.kegel.com/c10k.html)
- [Linux epoll man page](https://man7.org/linux/man-pages/man7/epoll.7.html)
- [Rust FFI Documentation](https://doc.rust-lang.org/nomicon/ffi.html)
- [Event-driven architecture patterns](https://en.wikipedia.org/wiki/Event-driven_architecture)

---

**Built with Rust 🦀 | Learning Systems Programming | Understanding Modern Server Architecture**