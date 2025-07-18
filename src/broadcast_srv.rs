use std::collections::HashMap;
use std::io::{Error, ErrorKind, Read, Result, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use log::{debug, error, info};

use crate::{Event, Operation, PeerRole, close, epoll_create, epoll_ctl, epoll_wait};

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
        println!("Broadcast server listening on {}", self.local_addr()?);
        self.register_peer(Operation::ADD, self.as_raw_fd(), PeerRole::Server)?;
        debug!("Server registered as entry for interest list in epoll");

        while !self.shutdown_signal.load(Ordering::Relaxed) {
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

        info!("Server shutting down gracefully");
        Ok(())
    }

    fn handle_notified_events(&mut self, events: &[Event]) -> Result<()> {
        for event in events {
            match event.role() {
                PeerRole::Server => {
                    debug!("Handlling server events");
                    match self.listener.accept() {
                        Ok((socket, addr)) => {
                            info!("new client connection from {}", addr);
                            socket.set_nonblocking(true)?;
                            let socket_fd = socket.as_raw_fd();
                            let identifier = self.next_client_id;

                            self.register_peer(
                                Operation::ADD,
                                socket_fd,
                                PeerRole::Client(identifier),
                            )?;

                            self.next_client_id += 1;
                            self.clients.insert(identifier, socket);
                        }
                        Err(e) => return Err(e),
                    }
                }
                PeerRole::Client(client_id) => {
                    debug!("Handling events for client with ID [{}]", client_id);
                    match event.event_type() {
                        0x1 => {
                            // read event -> broadcast to all client
                            let mut accumulated_data = Vec::new();
                            if let Some(client_soc) = self.clients.get_mut(&client_id) {
                                let mut buffer = vec![0u8; 4096];
                                loop {
                                    match client_soc.read(&mut buffer) {
                                        Ok(n) if n == 0 => {
                                            debug!(
                                                "Read 0 bytes - connection closed or no more data"
                                            );
                                            break;
                                        }
                                        Ok(n) => {
                                            debug!("Read {} bytes", n);
                                            accumulated_data.extend_from_slice(&buffer[..n]);
                                        }
                                        Err(e) if e.kind() == ErrorKind::WouldBlock => {
                                            debug!("WouldBlock - no more data available");
                                            break;
                                        }
                                        Err(e) => {
                                            error!("Read error: {}", e);
                                            return Err(e);
                                        }
                                    }
                                }
                            }
                            // at this stage we read all data and stored in buffer
                            // now broadcast to all except the one we received from
                            let formatted_message = format!(
                                "[Client_{}] {}",
                                client_id,
                                String::from_utf8_lossy(&accumulated_data)
                            );
                            info!("{}", formatted_message);
                            if self.clients.len() > 1 {
                                for (id, stream) in self.clients.iter_mut() {
                                    if *id != client_id {
                                        match stream.write(formatted_message.as_bytes()) {
                                            Ok(size) => {
                                                info!(
                                                    "{} bytes sent from client {} to {}",
                                                    size, client_id, id
                                                );
                                            }
                                            Err(_) => {
                                                error!("failed to send data to client: {}", id);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        0x2000 => {
                            // disconnect event
                            if let Some(client_socket) = self.clients.remove(&client_id) {
                                let fd = client_socket.as_raw_fd();
                                self.deregister_client(Operation::DEL, fd, client_id)?;
                            }
                        }
                        others => {
                            error!(
                                "received unknown events in clinet socket id: {}, event_type: {}",
                                client_id, others
                            );
                            if let Some(client_socket) = self.clients.remove(&client_id) {
                                let fd = client_socket.as_raw_fd();
                                self.deregister_client(Operation::DEL, fd, client_id)?;
                                info!(
                                    "client {} with address {:?} disconnected",
                                    client_id,
                                    client_socket.local_addr().unwrap()
                                );
                            }
                        }
                    }
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
    fn register_peer(&self, op: Operation, fd: i32, peer_role: PeerRole) -> Result<()> {
        let mut event = Event::new(peer_role).edge_trigerred().notify_read();

        if let PeerRole::Client(_) = &peer_role {
            event = event.notify_conn_close();
        }

        let res = unsafe { epoll_ctl(self.epfd, op.into(), fd, &raw mut event) };

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

    /// Register client's socket with the epoll instance
    fn deregister_client(&self, op: Operation, fd: i32, client_id: u32) -> Result<()> {
        let mut event = Event::new(PeerRole::Client(client_id))
            .edge_trigerred()
            .notify_read()
            .notify_conn_close();

        let res = unsafe { epoll_ctl(self.epfd, op.into(), fd, &raw mut event) };

        if res < 0 {
            return Err(Error::last_os_error());
        }

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
