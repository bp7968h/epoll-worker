//! Tcp client
//!
//! Connects to the specified address (from argument)
//! Sends messages to server from stdin
//! Prints messages from server to stdout
//!
//! Usage: RUST_LOG=info cargo run --example echo_server

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::{env, process, thread};

use log::{error, info};

fn main() -> io::Result<()> {
    env_logger::init();
    let Some(address) = env::args().nth(1) else {
        info!("Usage: Target address is required [./client 127.0.0.1:8080 ]");
        process::exit(1);
    };
    // Connect to the chat server
    let mut stream = TcpStream::connect(address)?;
    info!("Connected to chat server!");
    info!("Type messages and press Enter to send. Ctrl+C to quit.");

    // Clone stream for reading in separate thread
    let read_stream = stream.try_clone()?;

    // Spawn thread to handle incoming messages
    thread::spawn(move || {
        let mut reader = BufReader::new(read_stream);
        let mut buffer = String::new();

        loop {
            buffer.clear();
            match reader.read_line(&mut buffer) {
                Ok(0) => {
                    info!("Server disconnected");
                    break;
                }
                Ok(_) => {
                    // Print received message
                    print!(">> {}", buffer);
                    io::stdout().flush().unwrap();
                }
                Err(e) => {
                    error!("Error reading from server: {}", e);
                    break;
                }
            }
        }
    });

    // Main thread handles user input
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(input) => {
                if input.trim().is_empty() {
                    continue;
                }
                if input.as_str() == "--disconnect" {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    break;
                }
                let message = format!("{}\n", input);
                if let Err(e) = stream.write_all(message.as_bytes()) {
                    error!("Error sending message: {}", e);
                    break;
                }
            }
            Err(e) => {
                error!("Error reading input: {}", e);
                break;
            }
        }
    }

    info!("Client disconnecting...");
    Ok(())
}
