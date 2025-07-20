use std::{io::Result, net::TcpStream};

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
    fn on_connection(&mut self, client_id: u32, stream: &TcpStream) -> Result<()>;
    fn on_message(&mut self, client_id: u32, data: &[u8]) -> Result<HandlerAction>;
    fn on_disconnect(&mut self, client_id: u32) -> Result<()>;
}
