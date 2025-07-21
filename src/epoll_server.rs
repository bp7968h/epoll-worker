use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Read, Result, Write},
    net::{SocketAddr, TcpListener, ToSocketAddrs},
    os::fd::{AsRawFd, RawFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use log::{debug, error, info};

use crate::{
    Event, EventType, Operation, PeerRole,
    client_state::ClientState,
    close, epoll_create1, epoll_ctl, epoll_wait, fcntl,
    handler::{EventHandler, HandlerAction},
};

pub struct EpollServer<H> {
    listener: TcpListener,
    epfd: i32,
    clients: HashMap<u64, ClientState>,
    shutdown_signal: Arc<AtomicBool>,
    next_client_id: u64,
    handler: H,
}

impl<H: EventHandler> EpollServer<H> {
    pub fn new<A: ToSocketAddrs>(addr: A, handler: H) -> Result<Self> {
        let listener = TcpListener::bind(addr)?;
        if let Err(e) = listener.set_nonblocking(true) {
            error!("Failed to set listener to non blocking");
            return Err(e);
        }

        let result = unsafe { epoll_create1(0) };

        if result < 0 {
            error!("Failed to create epoll instance");
            return Err(Error::last_os_error());
        }

        // let fctl_res = unsafe { fcntl(result, 1) };
        // if fctl_res > 0 {
        //     let _ = unsafe { fcntl(result, 2, fctl_res | 0x1) };
        // }

        debug!("Epoll instance created with efd: `{}`", result);
        Ok(EpollServer {
            listener,
            epfd: result,
            clients: HashMap::new(),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            next_client_id: 1,
            handler,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        info!("Server listening on {}", self.local_addr()?,);

        let event_bitmask: i32 = EventType::Epollin as i32 | EventType::Epolloneshot as i32;
        let epoll_event = Event::new(event_bitmask as u32, PeerRole::Server);
        self.register_interest(Operation::Add, self.as_raw_fd(), epoll_event)?;

        while !self.shutdown_signal.load(Ordering::Relaxed) {
            let mut notified_events = Vec::with_capacity(1024);
            self.poll(&mut notified_events, Some(3000))?;

            if notified_events.is_empty() {
                continue;
            }

            debug!(
                "Now handling {} events received from epoll",
                notified_events.len()
            );
            let _ = self.handle_notified_events(&notified_events)?;
        }
        Ok(())
    }

    fn handle_notified_events(&mut self, events: &[Event]) -> Result<()> {
        debug!("All Events: {:?}", events);
        for event in events {
            debug!("Single Event Handling: {:?}", event);
            match event.role() {
                PeerRole::Server => {
                    debug!("[Server] Handling event for server");
                    if let Err(e) = self.accept_new_client() {
                        error!("[Server] Error accepting new client: {}", e);
                    }
                    let event_bitmask: i32 =
                        EventType::Epollin as i32 | EventType::Epolloneshot as i32;
                    let epoll_event = Event::new(event_bitmask as u32, PeerRole::Server);
                    self.register_interest(Operation::Mod, self.as_raw_fd(), epoll_event)?;
                }
                PeerRole::Client(client_id) => {
                    debug!("Handling event for client with ID [{}]", client_id);
                    let mut should_disconnect = None;
                    let event_type = event.event_type() as i32;
                    let read_event = EventType::Epollin as i32;
                    let write_event = EventType::Epollout as i32;
                    let oneshot_event = EventType::Epolloneshot as i32;
                    if let Some(client) = self.clients.get_mut(&client_id) {
                        let stream_fd = client.as_raw_fd();
                        if event_type & read_event == read_event {
                            debug!("READ BLOCK");
                            // Handle read
                            Self::handle_client_read(client)?;
                            let epoll_event = if self.handler.is_data_complete(client.read_buf()) {
                                let bitmask = write_event | oneshot_event;
                                Event::new(bitmask as u32, PeerRole::Client(client_id))
                            } else {
                                let bitmask = read_event | oneshot_event;
                                Event::new(bitmask as u32, PeerRole::Client(client_id))
                            };
                            self.register_interest(Operation::Mod, stream_fd, epoll_event)?;
                        } else if event_type & write_event == write_event {
                            debug!("WRITE BLOCK");
                            // Handle write
                            self.handle_client_message(client_id)?;
                        } else {
                            debug!("DISCONNECTION BLOCK");
                            debug!(
                                "Client ID: {}, Cliend Addr: {} Client Fd: {}, Event Type: {:#x}",
                                client_id,
                                client.local_addr().unwrap(),
                                client.as_raw_fd(),
                                event_type,
                            );
                            should_disconnect = Some(client_id);
                        }
                    }

                    if let Some(id) = should_disconnect {
                        let _ = self.clients.remove(&id);
                    }
                }
            }
        }
        Ok(())
    }

    fn accept_new_client(&mut self) -> Result<()> {
        let (socket, addr) = self.listener.accept()?;
        info!("[Server] new client connection from {}", addr);

        socket.set_nonblocking(true)?;
        let socket_fd = socket.as_raw_fd();
        let identifier = self.next_client_id;

        if let Err(e) = self.handler.on_connection(identifier, &socket) {
            error!(
                "[Server] Handler on_connection failed for client {}: {}",
                identifier, e
            );
        }

        let bitmask: i32 = EventType::Epollin as i32 | EventType::Epolloneshot as i32;
        let epoll_event = Event::new(bitmask as u32, PeerRole::Client(identifier));
        self.register_interest(Operation::Add, socket_fd, epoll_event)?;

        self.next_client_id += 1;
        let new_client = ClientState::new(socket);
        self.clients.insert(identifier, new_client);
        Ok(())
    }

    fn handle_client_read(client_state: &mut ClientState) -> Result<()> {
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

    fn handle_client_message(&mut self, client_id: u64) -> Result<()> {
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

    fn handle_client_disconnection(&mut self, client_id: u64, _fd: i32) -> Result<()> {
        if let Some(client_socket) = self.clients.remove(&client_id) {
            let fd = client_socket.as_raw_fd();
            self.deregister_interest(Operation::Del, fd)?;
            debug!(
                "Deregistered client with id {} from interest list",
                client_id
            );

            self.handler.on_disconnect(client_id)?;
            info!(
                "client {} with address disconnected",
                client_id,
                // client_socket.local_addr().unwrap()
            );
        }

        Ok(())
    }

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

        debug!("Epoll polling timeout reached, received events {}", res);
        Ok(())
    }

    fn deregister_interest(&self, op: Operation, fd: i32) -> Result<()> {
        let res = unsafe { epoll_ctl(self.epfd, op.into(), fd, std::ptr::null_mut()) };

        if res < 0 {
            return Err(Error::last_os_error());
        }

        Ok(())
    }

    fn register_interest(&self, op: Operation, fd: i32, mut event: Event) -> Result<()> {
        debug!(
            "Registering peer [{:?}] with fd {} and event flags {:#x}",
            PeerRole::from(event.data()),
            fd,
            event.event_type()
        );
        let res = unsafe { epoll_ctl(self.epfd, op.into(), fd, &raw mut event) };

        if res < 0 {
            debug!(
                "Failed to register {:?} with events flags {:#x} to epoll interest list",
                PeerRole::from(event.data()),
                event.event_type()
            );
            return Err(Error::last_os_error());
        }

        debug!(
            "Successfully registered {:?} with events flags {:#x} to epoll interest list",
            PeerRole::from(event.data()),
            event.event_type()
        );
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

impl<H> Drop for EpollServer<H> {
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
