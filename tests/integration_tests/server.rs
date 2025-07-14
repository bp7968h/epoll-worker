use std::{net::TcpStream, thread, time::Duration};

use epoll_broadcaster::BroadCastSrv;

#[test]
fn test_server_accept_connection() {
    let mut server = BroadCastSrv::new("127.0.0.1:0").unwrap();
    let server_addr = server.local_addr().unwrap();

    let shutdown_signal = server.shutdown_signal();
    thread::spawn(move || {
        server.run().unwrap();
    });

    thread::sleep(Duration::from_millis(100));

    let stream = TcpStream::connect(server_addr);
    assert!(stream.is_ok(), "Should be able to connect to server");

    shutdown_signal.store(true, std::sync::atomic::Ordering::Relaxed);
}
