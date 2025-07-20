//! Basic HTTP server serving simple responses
//!
//! Usage: RUST_LOG=info cargo run --example http_server
//! Test with: curl http://localhost:8080

use epoll_worker::{EpollServer, EventHandler, HandlerAction};
use log::{debug, info};

const HTML: &'static str = r#"
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>EPOLL WORKER!</title>
  </head>
  <body>
    <h1>Hello!</h1>
    <p>Request sent from Epoll-worker</p>
  </body>
</html>
"#;

struct HttpHandler;

impl EventHandler for HttpHandler {
    fn on_connection(
        &mut self,
        client_id: u32,
        stream: &std::net::TcpStream,
    ) -> std::io::Result<()> {
        info!(
            "Client {} connected from {}",
            client_id,
            stream.local_addr()?
        );
        Ok(())
    }

    fn on_disconnect(&mut self, client_id: u32) -> std::io::Result<()> {
        info!("Client {} disconnected", client_id);
        Ok(())
    }

    fn on_message(&mut self, _client_id: u32, data: &[u8]) -> std::io::Result<HandlerAction> {
        let request = String::from_utf8_lossy(data);
        debug!("Request: {}", request);

        if request.starts_with("GET / HTTP/1.1") {
            let status_line = "HTTP/1.1 200 OK";
            let contents = HTML;
            let length = contents.len();
            let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

            return Ok(HandlerAction::Reply(response.as_bytes().to_vec()));
        }

        Ok(HandlerAction::Reply(
            b"HTTP/1.1 404 Not Found\r\n\r\n".to_vec(),
        ))
    }
}
fn main() -> std::io::Result<()> {
    env_logger::init();

    let handler = HttpHandler;
    let mut server = EpollServer::new("127.0.0.1:8080", handler)?;
    server.run()
}
