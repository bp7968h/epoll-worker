use std::collections::HashMap;
use std::io::{Error, Result};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use log::{debug, error, info};

use crate::{EPOLL_CTL_ADD, Event, PeerRole, close, epoll_create, epoll_ctl, epoll_wait};

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
            error!("Failed to create epoll instance");
            return Err(Error::last_os_error());
        }

        debug!("Epoll instance created with efd: `{}`", result);
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
        self.register_peer(EPOLL_CTL_ADD, self.as_raw_fd(), PeerRole::Server)?;
        debug!("Server registered as entry for interest list in epoll");

        loop {
            let mut notified_events = Vec::with_capacity(12);
            debug!("Waiting for events from epoll...");
            self.poll(&mut notified_events, None)?;
            debug!("Got {} events from epoll to handle", notified_events.len());

            if notified_events.is_empty() {
                debug!("no events received from epoll");
                continue;
            }

            let _ = self.handle_notified_events(&notified_events);
        }
    }

    fn handle_notified_events(&mut self, events: &[Event]) -> Result<()> {
        for event in events {
            match event.data() {
                0 => {
                    // 0 is registered as server always
                    debug!("Handlling server events");
                    match self.listener.accept() {
                        Ok((socket, addr)) => {
                            info!("new client connection from {}", addr);
                            socket.set_nonblocking(true)?;
                            let socket_fd = socket.as_raw_fd();
                            let identifier = self.next_client_id;

                            self.register_peer(
                                EPOLL_CTL_ADD,
                                socket_fd,
                                PeerRole::Client(identifier),
                            )?;

                            self.next_client_id += 1;
                            self.clients.insert(identifier, socket);
                        }
                        Err(e) => return Err(e),
                    }
                }
                client_id => {
                    debug!("Handling events for client with ID [{}]", client_id);
                    // except for 0 other are clinet, as clinet id start at 1
                }
            }
        }
        Ok(())
    }

    /// Poll the epoll instance for any event
    fn poll(&self, events: &mut Vec<Event>, timeout: Option<i32>) -> Result<()> {
        let epfd = self.epfd;
        let max_events = events.capacity() as i32;
        let timeout = timeout.unwrap_or(-1);

        let res = unsafe { epoll_wait(epfd, events.as_mut_ptr(), max_events, timeout) };

        if res < 0 {
            debug!("Failed to wait for events from epoll wait");
            return Err(Error::last_os_error());
        }

        unsafe {
            events.set_len(res as usize);
        }

        debug!("Successfully got events from epoll wait");
        Ok(())
    }

    /// Register peer (server or client) to epoll interest list
    ///
    /// Some flags are different while registering to epoll interest list.
    /// This depends on the type of role you have, which needs to be passed as arguments
    fn register_peer(&self, op: i32, fd: i32, peer_role: PeerRole) -> Result<()> {
        let mut event = Event::new(peer_role).edge_trigerred().notify_read();

        if let PeerRole::Client(_) = &peer_role {
            event = event.notify_conn_close();
        }

        let res = unsafe { epoll_ctl(self.epfd, op, fd, &raw mut event) };

        if res < 0 {
            debug!("Failed to register {:?} to epoll interest list", peer_role);
            return Err(Error::last_os_error());
        }

        debug!(
            "Successfully registered {:?} to epoll interest list",
            peer_role
        );
        Ok(())
    }

    fn as_raw_fd(&self) -> RawFd {
        self.listener.as_raw_fd()
    }
}

impl Drop for BroadCastSrv {
    fn drop(&mut self) {
        let res = unsafe { close(self.epfd) };

        if res < 0 {
            error!("failed to close epoll instance, {}", Error::last_os_error());
        }

        debug!(
            "Notified kernel to close the epoll instance with fd {}",
            self.epfd
        );
    }
}
