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
        let event_bitmask: i32 = EventType::Epollin as i32 | EventType::Epolloneshot as i32;
        let epoll_event = Event::new(event_bitmask as u32, PeerRole::Server);
        self.epoll.add_interest(self.as_raw_fd(), epoll_event)?;

        let mut total_r = 0;
        let mut total_w = 0;
        let mut total_d = 0;
        while !self.shutdown_signal.load(Ordering::Relaxed) {
            let mut notified_events = Vec::with_capacity(1024);
            self.epoll.wait(&mut notified_events, timeout)?;

            if notified_events.is_empty() {
                continue;
            }

            debug!(
                "Now handling {} events received from epoll",
                notified_events.len()
            );

            self.handle_events(&notified_events, &mut total_r, &mut total_w, &mut total_d)?;
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
    fn handle_events(
        &mut self,
        events: &[Event],
        total_r: &mut i32,
        total_w: &mut i32,
        total_d: &mut i32,
    ) -> Result<()> {
        for event in events {
            match event.role() {
                PeerRole::Server => {
                    if let Err(e) = self.accept_new_client() {
                        error!("Error accepting new client: {}", e);
                    }
                    let event_bitmask: i32 =
                        EventType::Epollin as i32 | EventType::Epolloneshot as i32;
                    let epoll_event = Event::new(event_bitmask as u32, PeerRole::Server);
                    self.epoll.modify_interest(self.as_raw_fd(), epoll_event)?;
                }
                PeerRole::Client(id) => {
                    let mut should_disconnect = None;
                    let event_type = event.event_type() as i32;
                    let read_event = EventType::Epollin as i32;
                    let write_event = EventType::Epollout as i32;
                    let oneshot_event = EventType::Epolloneshot as i32;
                    if let Some(client) = self.clients.get_mut(&id) {
                        let stream_fd = client.as_raw_fd();
                        if event_type & read_event == read_event {
                            // Handle read
                            Self::handle_read(client)?;
                            // validate if we received all the data
                            let epoll_event = if self.handler.is_data_complete(client.read_buf()) {
                                // Data complete: wait for fd to be writable
                                let bitmask = write_event | oneshot_event;
                                Event::new(bitmask as u32, PeerRole::Client(id))
                            } else {
                                // Data incomplete: wait for more data to be read
                                let bitmask = read_event | oneshot_event;
                                Event::new(bitmask as u32, PeerRole::Client(id))
                            };
                            self.epoll.modify_interest(stream_fd, epoll_event)?;
                            *total_r += 1;
                        } else if event_type & write_event == write_event {
                            // Handle write
                            self.handle_message(id)?;
                            should_disconnect = Some(id);
                            *total_w += 1;
                        } else {
                            debug!(
                                "Disconnecting Client: id/fd({}), addr({}), event_type({:#x})",
                                id,
                                client.local_addr().unwrap(),
                                event_type,
                            );
                            should_disconnect = Some(id);
                        }
                    }

                    if let Some(id) = should_disconnect {
                        self.handle_disconnection(id)?;
                        *total_d += 1;
                    }
                }
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
    fn handle_read(client_state: &mut ClientState) -> Result<()> {
        let mut buffer = vec![0u8; 4096];
        let mut total_read = 0;
        loop {
            match client_state.stream_mut().read(&mut buffer) {
                Ok(0) => {
                    debug!("Client closed connection or no more data to read");
                    break;
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
                    error!("Read error: {}", e);
                    return Err(e);
                }
            }
        }
        client_state
            .read_buf_mut()
            .extend_from_slice(&buffer[..total_read]);
        Ok(())
    }

    fn handle_message(&mut self, client_id: u64) -> Result<()> {
        if let Some(client) = self.clients.get_mut(&client_id) {
            let mut action: Option<HandlerAction> = None;
            if !client.read_buf().is_empty() {
                match self.handler.on_message(client_id, client.read_buf()) {
                    Ok(returned_action) => {
                        action = Some(returned_action);
                        client.read_buf_mut().clear();
                    }
                    Err(e) => {
                        error!("Handler on_message error for client {}: {}", client_id, e);
                        return Err(e);
                    }
                }
            }

            // Process handler action
            if let Some(action_to_handle) = action {
                match action_to_handle {
                    HandlerAction::Broadcast(data) => {}
                    HandlerAction::Reply(data) => {
                        let written_bytes = client.write(&data)?;
                        debug!(
                            "Sent {} bytes Client Id: {} at Client Address: {}",
                            written_bytes,
                            client_id,
                            client.local_addr().unwrap()
                        );
                        client.shutdown()?;
                    }
                    HandlerAction::SendTo {
                        target_client_id,
                        data,
                    } => {}
                    HandlerAction::SendToAll(data) => {}
                    HandlerAction::None => (),
                }
            }
        }
        Ok(())
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
