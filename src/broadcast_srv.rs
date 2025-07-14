use std::collections::HashMap;
use std::io::{Error, Result};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use log::{debug, error};

use crate::epoll_create;

/// Broadcast server instance
///
/// This contains the owned TcpListener, epoll file descriptor, shutdown trigger, clinets stream
pub struct BroadCastSrv {
    listener: TcpListener,
    epfd: i32,
    next_client_id: u32,
    clients: HashMap<u32, TcpStream>,
    shutdown_signal: Arc<AtomicBool>,
}

impl BroadCastSrv {
    /// Create new instance of BroadCastSrv
    ///
    /// Binds the listener to passed address
    /// creates the epoll instance and saves the fd
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let listener = TcpListener::bind(addr)?;

        let result = unsafe { epoll_create(1) };

        if result < 0 {
            error!("failed to create epoll instance");
            return Err(Error::last_os_error());
        }

        debug!("epoll instance created with efd: `{}`", result);
        Ok(BroadCastSrv {
            listener: listener,
            epfd: result,
            next_client_id: 1,
            clients: HashMap::new(),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn shutdown_signal(&self) -> Arc<AtomicBool> {
        self.shutdown_signal.clone()
    }

    /// Shutdown instance of BroadCastSrv
    fn shutdown(&self) {
        self.shutdown_signal.store(true, Ordering::Relaxed);
    }

    /// Returns the local addres the server is binded to
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Runs BroadCastSrv event loop
    ///
    /// Registers the listeners file description with the epoll instance,
    /// polls for the notification from kernel
    /// handles the respective events that the kernel notifies
    pub fn run(&mut self) -> Result<()> {
        Ok(())
    }
}
