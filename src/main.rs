use std::io::Result;

use epoll_broadcaster::BroadCastSrv;

fn main() -> Result<()> {
    let mut server = BroadCastSrv::new("127.0.0.1:8080")?;

    server.run()?;

    Ok(())
}
