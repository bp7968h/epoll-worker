mod epoll;
pub(crate) use epoll::*;

mod epoll_server;
mod handler;

mod client_state;

pub use epoll_server::EpollServer;
pub use handler::{EventHandler, HandlerAction};
