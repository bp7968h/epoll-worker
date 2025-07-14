use std::io;

fn main() -> io::Result<()> {
  let server = BroadCastSrv::new("127.0.0.1:8080")?;

  server.run()?
}
