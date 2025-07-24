//! Real-time chat server that broadcasts messages to all connected clients
//!
//! Usage: RUST_LOG=info cargo run --example broadcast_server
//! Connect with: <telnet localhost 8080> or <client provided in example>

use env_logger;
use epoll_worker::{EpollServer, EventHandler, HandlerAction};
use log::info;

struct BroadcastHandler;

impl EventHandler for BroadcastHandler {
    fn on_connection(
        &mut self,
        client_id: u64,
        stream: &std::net::TcpStream,
    ) -> std::io::Result<()> {
        info!(
            "Client {} connected from {}",
            client_id,
            stream.local_addr()?
        );
        Ok(())
    }

    fn on_disconnect(&mut self, client_id: u64) -> std::io::Result<()> {
        info!("Client {} disconnected", client_id);
        Ok(())
    }

    fn on_message(&mut self, client_id: u64, data: &[u8]) -> std::io::Result<HandlerAction> {
        let message = format!("[Client_{}] {}", client_id, String::from_utf8_lossy(data));
        Ok(HandlerAction::Broadcast(message.into_bytes()))
    }
    fn is_data_complete(&mut self, _data: &[u8]) -> bool {
        true
    }
}

fn main() -> std::io::Result<()> {
    env_logger::init();

    let handler = BroadcastHandler;
    let mut server = EpollServer::new("127.0.0.1:8080", handler)?;
    server.run(None)
}
