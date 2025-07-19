use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::Duration,
};

use crate::common::{create_clients, start_test_server};

#[test]
fn test_server_accept_connection() {
    let (mut server, addr, shutdown_signal) = start_test_server();
    thread::spawn(move || {
        server.run().unwrap();
    });

    thread::sleep(Duration::from_millis(100));

    let stream = TcpStream::connect(addr);
    assert!(stream.is_ok(), "Should be able to connect to server");

    shutdown_signal.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[test]
fn test_message_broadcasting() {
    let (mut server, addr, shutdown_signal) = start_test_server();

    thread::spawn(move || {
        server.run().unwrap();
    });

    thread::sleep(Duration::from_millis(100));

    let mut clients = create_clients(addr, 2);
    thread::sleep(Duration::from_millis(100));

    clients[0].write_all(b"Hello from client 1\n").unwrap();
    thread::sleep(Duration::from_millis(100));

    let mut buffer = [0; 1024];
    let n = clients[1].read(&mut buffer).unwrap();
    let received = String::from_utf8_lossy(&buffer[..n]);

    assert!(
        received.contains("Hello from client 1"),
        "Client 2 should receive message from Client 1. Got: {}",
        received
    );

    shutdown_signal.store(true, std::sync::atomic::Ordering::Relaxed);
}
