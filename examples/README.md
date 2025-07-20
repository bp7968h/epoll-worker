# Examples

This directory contains example implementations showing different server types built with the epoll worker.

## Running Examples

### Servers

```bash
# Chat/broadcast server
RUST_LOG=info cargo run --example broadcast_server

# Echo server
RUST_LOG=info cargo run --example echo_server

# Basic HTTP server
RUST_LOG=info cargo run --example http_server
```

### Client

```bash
# Tcp Client
RUST_LOG=info cargo run --example client <server_address>
```