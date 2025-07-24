//! Echo server that sends back whatever you type
//!
//! Usage: RUST_LOG=info cargo run --example echo_server

use epoll_worker::{EpollServer, EventHandler, HandlerAction};
use log::info;

struct EchoHandler;

impl EventHandler for EchoHandler {
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

    fn on_message(
        &mut self,
        _client_id: u64,
        data: &[u8],
    ) -> std::io::Result<epoll_worker::HandlerAction> {
        Ok(HandlerAction::Reply(data.to_vec()))
    }

    fn is_data_complete(&mut self, _data: &[u8]) -> bool {
        true
    }
}
fn main() -> std::io::Result<()> {
    env_logger::init();

    let handler = EchoHandler;
    let mut server = EpollServer::new("127.0.0.1:8080", handler)?;
    server.run(None)
}
