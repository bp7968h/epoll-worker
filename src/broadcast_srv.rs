use std::collections::HashMap;
use std::io::{Error, Result};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use log::{debug, error, info};

use crate::{EPOLL_CTL_ADD, Event, close, epoll_create, epoll_ctl, epoll_wait};

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
        info!("Chat server listening on {}", self.local_addr()?);
        self.register_server(EPOLL_CTL_ADD, self.epfd, 0)?;

        loop {
            let mut notified_events = Vec::with_capacity(12);
            self.poll(&mut notified_events, None)?;

            if notified_events.is_empty() {
                info!("no events received from epoll");
                continue;
            }

            let _ = self.handle_notified_events(&notified_events);
        }
    }

    fn handle_notified_events(&mut self, event: &[Event]) -> Result<()> {
        Ok(())
    }

    /// Poll the epoll instance for any event
    fn poll(&self, events: &mut Vec<Event>, timeout: Option<i32>) -> Result<()> {
        let epfd = self.epfd;
        let max_events = events.capacity() as i32;
        let timeout = timeout.unwrap_or(-1);

        let res = unsafe { epoll_wait(epfd, events.as_mut_ptr(), max_events, timeout) };

        if res < 0 {
            return Err(Error::last_os_error());
        }

        unsafe {
            events.set_len(res as usize);
        }

        Ok(())
    }

    /// Register server's listener with the epoll instance
    fn register_server(&self, op: i32, fd: i32, identifier: u32) -> Result<()> {
        let mut event = Event::new(identifier).edge_trigerred().notify_read();

        let res = unsafe { epoll_ctl(self.epfd, op, fd, &raw mut event) };

        if res < 0 {
            return Err(Error::last_os_error());
        }

        Ok(())
    }
}

impl Drop for BroadCastSrv {
    fn drop(&mut self) {
        let res = unsafe { close(self.epfd) };

        if res < 0 {
            error!("failed to close epoll instance, {}", Error::last_os_error());
        }
    }
}
