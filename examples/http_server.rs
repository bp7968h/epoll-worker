//! Basic HTTP server serving simple responses
//!
//! Usage: RUST_LOG=info cargo run --example http_server
//! Test with: curl http://localhost:8080

use epoll_worker::{EpollServer, EventHandler, HandlerAction};
use log::{debug, info};

const HTML_200: &'static str = r#"
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

const HTML_404: &'static str = r#"
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <title>EPOLL WORKER!</title>
  </head>
  <body>
    <h1>Oops!</h1>
    <p>Sorry, I don't know what you're asking for.</p>
  </body>
</html>
"#;

struct HttpHandler;

impl EventHandler for HttpHandler {
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

    fn on_message(&mut self, _client_id: u64, data: &[u8]) -> std::io::Result<HandlerAction> {
        let request = String::from_utf8_lossy(data);
        let (status_line, contents) = match request.lines().next() {
            Some(first_line) => {
                if first_line.starts_with("GET / HTTP/1.1") {
                    ("HTTP/1.1 200 OK", HTML_200)
                } else if first_line.starts_with("GET ") && first_line.ends_with(" HTTP/1.1") {
                    ("HTTP/1.1 404 NOT FOUND", HTML_404)
                } else {
                    ("HTTP/1.1 400 BAD REQUEST", HTML_404)
                }
            }
            None => ("HTTP/1.1 400 BAD REQUEST", HTML_404),
        };
        let length = contents.len();

        let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");

        Ok(HandlerAction::Reply(response.as_bytes().to_vec()))
    }

    fn is_data_complete(&mut self, data: &[u8]) -> bool {
        let data_str = String::from_utf8_lossy(data);
        debug!("Received: {}", data_str);
        let mut lines = data_str.lines();
        if let Some(line) = lines.next() {
            if let Some(method) = line.split(" ").nth(0) {
                match method {
                    "GET" | "DELETE" => return true,
                    _ => (),
                }
            }
        }

        if let Some(content_len) = lines.find(|l| l.to_lowercase().starts_with("content-length: "))
        {
            if let Some(len) = content_len.to_lowercase().strip_prefix("content-length: ") {
                let is_valid = data.len()
                    > len
                        .parse::<usize>()
                        .expect("content-length to be valid number");
                debug!("Content-Length Validate: {}/{}", is_valid, len);
                return is_valid;
            }
        }
        false
    }
}
fn main() -> std::io::Result<()> {
    env_logger::init();

    let handler = HttpHandler;
    let mut server = EpollServer::new("127.0.0.1:8080", handler)?;
    server.run(None)
}
