use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::Duration,
};

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

#[test]
fn test_message_broadcasting() {
    let mut server = BroadCastSrv::new("127.0.0.1:0").unwrap();
    let server_addr = server.local_addr().unwrap();

    let shutdown_signal = server.shutdown_signal();
    thread::spawn(move || {
        server.run().unwrap();
    });

    thread::sleep(Duration::from_millis(100));

    let mut client1 = TcpStream::connect(server_addr).unwrap();
    let mut client2 = TcpStream::connect(server_addr).unwrap();
    thread::sleep(Duration::from_millis(100));

    client1.write_all(b"Hello from client 1\n").unwrap();
    thread::sleep(Duration::from_millis(100));

    let mut buffer = [0; 1024];
    let n = client2.read(&mut buffer).unwrap();
    let received = String::from_utf8_lossy(&buffer[..n]);

    assert!(
        received.contains("Hello from client 1"),
        "Client 2 should receive message from Client 1. Got: {}",
        received
    );

    shutdown_signal.store(true, std::sync::atomic::Ordering::Relaxed);
}
