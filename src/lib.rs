mod epoll;
mod ffi;
pub(crate) use epoll::*;
pub(crate) use ffi::*;

mod epoll_server;
mod handler;

mod client_state;

pub use epoll_server::EpollServer;
pub use handler::{EventHandler, HandlerAction};

/// This is a helper macro to do syscall
///
/// Basically we want to call function with zero, one or more arguments
/// So we have the below format to match
///
///     epoll_create1(1) or epoll_ctl(1,1,1, &raw mut Event)
///
/// we do exactly that in the macro that is
///
///     identifier bracket_open zero_or_more_expression bracker_close
///
/// Note: In a function call trailing comman in arguments is ignored
/// if atleast one argument is present by Rust
#[macro_export]
macro_rules! ep_syscall {
    ($epoll_fn:ident ( $($arg:expr),* )) => {{
        let result = unsafe { crate::ffi::$epoll_fn($($arg,)*) };

        if result < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(result)
        }
    }};
}
