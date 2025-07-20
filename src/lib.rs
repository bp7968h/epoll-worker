mod epoll;
pub(crate) use epoll::*;

mod epoll_server;
mod handler;

pub use epoll_server::EpollServer;
pub use handler::{EventHandler, HandlerAction};
