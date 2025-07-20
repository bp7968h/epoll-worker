use std::{
    net::{SocketAddr, TcpStream},
    sync::{Arc, atomic::AtomicBool},
};

use epoll_worker::{EpollServer, EventHandler};

pub fn start_test_server<H: EventHandler>(
    handler: H,
) -> (EpollServer<H>, SocketAddr, Arc<AtomicBool>) {
    let server = EpollServer::new("127.0.0.1:0", handler).unwrap();
    let addr = server.local_addr().unwrap();
    let shutdown = server.shutdown_signal();

    (server, addr, shutdown)
}

pub fn create_clients(addr: SocketAddr, count: usize) -> Vec<TcpStream> {
    (0..count)
        .map(|_| TcpStream::connect(addr).unwrap())
        .collect()
}
