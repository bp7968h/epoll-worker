use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Result},
    net::{SocketAddr, TcpListener, ToSocketAddrs},
    os::fd::{AsRawFd, RawFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use log::{debug, error, info};

use crate::{
    Epoll, Event, EventType, PeerRole,
    client_state::ClientState,
    handler::{EventHandler, HandlerAction},
};

/// Server instance that listens for request
pub struct EpollServer<H> {
    listener: TcpListener,
    epoll: Epoll,
    clients: HashMap<u64, ClientState>,
    shutdown_signal: Arc<AtomicBool>,
    handler: H,
}

impl<H: EventHandler> EpollServer<H> {
    /// Create new Server instance
    ///
    /// Requires valid address and handler that will be called
    pub fn new<A: ToSocketAddrs>(addr: A, handler: H) -> Result<Self> {
        let listener = TcpListener::bind(addr)?;
        if let Err(e) = listener.set_nonblocking(true) {
            error!("Failed to set listener to non blocking");
            return Err(e);
        }

        let epoll = Epoll::new()?;

        debug!("Epoll instance created with efd: `{}`", epoll.fd());
        Ok(EpollServer {
            listener,
            epoll,
            clients: HashMap::new(),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            handler,
        })
    }

    /// Run the server instance
    ///
    /// Registers the listener's file descriptor to epoll insterest list
    /// where we get notification for read events in Edge-Triggered manner.
    /// Continously look for the events, and timeout if provided otherwise
    /// uses `1000` as the default timeout
    pub fn run(&mut self, timeout: Option<i32>) -> Result<()> {
        info!("Server listening on {}", self.local_addr()?,);
        // let event_bitmask: i32 = EventType::Epollin as i32 | EventType::Epolloneshot as i32;
        let event_bitmask: i32 = EventType::Epollin as i32 | EventType::Epollet as i32;
        let epoll_event = Event::new(event_bitmask as u32, PeerRole::Server);
        self.epoll.add_interest(self.as_raw_fd(), epoll_event)?;

        while !self.shutdown_signal.load(Ordering::Relaxed) {
            let mut notified_events = Vec::with_capacity(1024);
            self.epoll.wait(&mut notified_events, timeout)?;

            println!("Total Clients: {:?}", self.clients.len());

            if notified_events.is_empty() {
                continue;
            }

            debug!(
                "Now handling {} events received from epoll",
                notified_events.len()
            );

            self.handle_events(&notified_events)?;
        }
        Ok(())
    }

    /// Handle notified events from epoll
    ///
    /// Based on type of event received we decide how we want to handle those request
    /// Type can either be `Client` or `Server`
    /// Serve:
    ///     We are only interested in read events which means new connection
    /// Client:
    ///     First interested in read event, and based on the data that we received
    ///     we can to decide wheather to keep on reading or switch to write events
    fn handle_events(&mut self, events: &[Event]) -> Result<()> {
        for event in events {
            match event.role() {
                PeerRole::Server => loop {
                    match self.accept_new_client() {
                        Ok(()) => continue,
                        Err(e) if e.kind() == ErrorKind::WouldBlock => {
                            debug!("Drained all pending connections");
                            break;
                        }
                        Err(e) => {
                            error!("Error accepting new client: {}", e);
                        }
                    }
                },
                PeerRole::Client(id) => {
                    let event_type = event.event_type() as i32;
                    let read_event = EventType::Epollin as i32;
                    let write_event = EventType::Epollout as i32;
                    if let Some(client) = self.clients.get_mut(&id) {
                        let mut should_disconnect = false;
                        let mut need_interest_update = false;

                        if event_type & read_event == read_event {
                            match Self::handle_read(client) {
                                Ok(()) => {
                                    if self.handler.is_data_complete(client.read_buf()) {
                                        match self.handler.on_message(id, client.read_buf()) {
                                            Ok(action) => {
                                                client.read_buf_mut().clear();
                                                self.handle_action(id, action)?;
                                            }
                                            Err(e) => {
                                                error!(
                                                    "Handler `on_message` error for client {}: {}",
                                                    id, e
                                                );
                                                should_disconnect = true;
                                            }
                                        }
                                    }
                                }
                                Err(_) => should_disconnect = true,
                            }
                        }

                        if event_type & write_event == write_event {
                            if let Some(client) = self.clients.get_mut(&id) {
                                match client.flush_writes() {
                                    Ok(true) => {
                                        // All data written, remove write interest
                                        need_interest_update = true;
                                    }
                                    Ok(false) => {
                                        // More data to write, keep write interest
                                    }
                                    Err(_) => should_disconnect = true,
                                }
                            }
                        }

                        if need_interest_update && !should_disconnect {
                            self.update_client_interests(id)?;
                        }

                        if should_disconnect {
                            self.handle_disconnection(id)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_action(&mut self, originating_client_id: u64, action: HandlerAction) -> Result<()> {
        match action {
            HandlerAction::Reply(data) => {
                if let Some(client) = self.clients.get_mut(&originating_client_id) {
                    client.queue_write(data);
                    self.update_client_interests(originating_client_id)?;
                }
            }
            HandlerAction::Broadcast(data) => {
                // Send to all clients except the sender
                let client_ids: Vec<u64> = self.clients.keys().copied().collect();
                for client_id in client_ids {
                    if client_id != originating_client_id {
                        if let Some(client) = self.clients.get_mut(&client_id) {
                            client.queue_write(data.clone());
                            self.update_client_interests(client_id)?;
                        }
                    }
                }
            }
            HandlerAction::SendTo {
                target_client_id,
                data,
            } => {
                if let Some(client) = self.clients.get_mut(&(target_client_id as u64)) {
                    client.queue_write(data);
                    self.update_client_interests(target_client_id as u64)?;
                }
            }
            HandlerAction::SendToAll(data) => {
                // Send to all clients including sender
                let client_ids: Vec<u64> = self.clients.keys().copied().collect();
                for client_id in client_ids {
                    if let Some(client) = self.clients.get_mut(&client_id) {
                        client.queue_write(data.clone());
                        self.update_client_interests(client_id)?;
                    }
                }
            }
            HandlerAction::None => (),
        }
        Ok(())
    }

    fn update_client_interests(&mut self, client_id: u64) -> Result<()> {
        if let Some(client) = self.clients.get_mut(&client_id) {
            let fd = client.as_raw_fd();

            let mut new_interests = EventType::Epollin as i32 | EventType::Epollet as i32;

            if client.has_pending_writes() {
                new_interests |= EventType::Epollout as i32;
            }

            let new_interests = new_interests as u32;
            if client.current_interests() != new_interests {
                let epoll_event = Event::new(new_interests, PeerRole::Client(client_id));
                self.epoll.modify_interest(fd, epoll_event)?;
                client.set_current_interests(new_interests);
            }
        }

        Ok(())
    }

    /// Accept tcp connection from clients
    ///
    /// Add interest for read events to epoll interest list
    /// Uses the fd as the id for client while storing in map
    fn accept_new_client(&mut self) -> Result<()> {
        let (socket, addr) = self.listener.accept()?;

        socket.set_nonblocking(true)?;
        let socket_fd = socket.as_raw_fd();
        // use the file descriptor as the id for the client
        // this is safe because fd is unique and we remove client
        // from clients immediately, if we ever received disconnection
        let identifier = socket_fd as u64;

        if let Err(e) = self.handler.on_connection(identifier, &socket) {
            error!(
                "Handler `on_connection` failed for client id({}) addr({}): {}",
                identifier, addr, e
            );
        }

        let bitmask: i32 = EventType::Epollin as i32 | EventType::Epolloneshot as i32;
        let epoll_event = Event::new(bitmask as u32, PeerRole::Client(identifier));
        self.epoll.add_interest(socket_fd, epoll_event)?;

        let new_client = ClientState::new(socket);
        self.clients.insert(identifier, new_client);
        Ok(())
    }

    /// Handles data reading from file TcpStream
    ///
    /// Read until we exhaust the kernel buffer or we get all the bytes
    fn handle_read(client_state: &mut ClientState) -> Result<usize> {
        let mut buffer = vec![0u8; 4096];
        let mut total_read = 0;
        loop {
            match client_state.stream_mut().read(&mut buffer) {
                Ok(0) => {
                    debug!("Client closed connection or no more data to read");
                    return Ok(0);
                }
                Ok(n) => {
                    debug!("Read {} bytes", n);
                    client_state.read_buf_mut().extend_from_slice(&buffer[..n]);
                    total_read += n;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {
                    debug!(
                        "Drained the kernel's buffer (total read: {} bytes)",
                        total_read
                    );
                    break;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(total_read)
    }

    fn handle_disconnection(&mut self, id: u64) -> Result<()> {
        if let Some(client_socket) = self.clients.remove(&id) {
            let fd = client_socket.as_raw_fd();
            self.epoll.remove_interest(fd)?;

            self.handler.on_disconnect(id)?;
            info!("Client disconnected addr({})", id,);
        }

        Ok(())
    }

    pub fn shutdown_signal(&self) -> Arc<AtomicBool> {
        self.shutdown_signal.clone()
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr()
    }

    fn as_raw_fd(&self) -> RawFd {
        self.listener.as_raw_fd()
    }
}
