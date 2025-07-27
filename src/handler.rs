use std::{io::Result, net::TcpStream};

use crate::epoll_server::ClientId;

pub enum HandlerAction {
    Broadcast(Vec<u8>),
    Reply(Vec<u8>),
    SendTo {
        target_client_id: u32,
        data: Vec<u8>,
    },
    SendToAll(Vec<u8>),
    None,
}

pub trait EventHandler {
    fn on_connection(&mut self, client_id: ClientId, stream: &TcpStream) -> Result<()>;
    fn on_message(&mut self, client_id: ClientId, data: &[u8]) -> Result<HandlerAction>;
    fn on_disconnect(&mut self, client_id: ClientId) -> Result<()>;
    fn is_data_complete(&mut self, data: &[u8]) -> bool;
}
