use std::{
    collections::HashMap,
    io::{Error, ErrorKind, Read, Result, Write},
    net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    os::fd::{AsRawFd, RawFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use log::{debug, error, info};

use crate::{
    Event, EventType, Operation, PeerRole, close, epoll_create, epoll_ctl, epoll_wait,
    handler::{EventHandler, HandlerAction},
};

pub struct EpollServer<H> {
    listener: TcpListener,
    epfd: i32,
    next_client_id: u32,
    clients: HashMap<u32, TcpStream>,
    shutdown_signal: Arc<AtomicBool>,
    handler: H,
}

impl<H: EventHandler> EpollServer<H> {
    pub fn new<A: ToSocketAddrs>(addr: A, handler: H) -> Result<Self> {
        let listener = TcpListener::bind(addr)?;

        let result = unsafe { epoll_create(1) };

        if result < 0 {
            error!("Failed to create epoll instance");
            return Err(Error::last_os_error());
        }

        debug!("Epoll instance created with efd: `{}`", result);
        Ok(EpollServer {
            listener,
            epfd: result,
            next_client_id: 1,
            clients: HashMap::new(),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
            handler,
        })
    }

    pub fn shutdown_signal(&self) -> Arc<AtomicBool> {
        self.shutdown_signal.clone()
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.listener.local_addr()
    }

    pub fn run(&mut self) -> Result<()> {
        println!("Server listening on {}", self.local_addr()?,);
        self.register_peer(Operation::Add, self.as_raw_fd(), PeerRole::Server)?;

        while !self.shutdown_signal.load(Ordering::Relaxed) {
            let mut notified_events = Vec::with_capacity(12);
            self.poll(&mut notified_events, Some(1000))?;

            if notified_events.is_empty() {
                continue;
            }

            debug!(
                "Now handling {} events received from epoll",
                notified_events.len()
            );
            let _ = self.handle_notified_events(&notified_events);
        }
        Ok(())
    }

    fn handle_notified_events(&mut self, events: &[Event]) -> Result<()> {
        for event in events {
            match event.role() {
                PeerRole::Server => self.accept_new_client()?,
                PeerRole::Client(client_id) => {
                    debug!("Handling events for client with ID [{}]", client_id);
                    match event.event_type() {
                        0x1 => self.handle_client_message(client_id)?,
                        0x2000 => self.handle_client_disconnection(client_id)?,
                        others => {
                            error!(
                                "received unknown events in clinet socket id: {}, event_type: {}",
                                client_id, others as u32
                            );
                            self.handle_client_disconnection(client_id)?
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn accept_new_client(&mut self) -> Result<()> {
        debug!("Handlling server events");
        let (socket, addr) = self.listener.accept()?;
        info!("new client connection from {}", addr);
        socket.set_nonblocking(true)?;

        let socket_fd = socket.as_raw_fd();
        let identifier = self.next_client_id;

        // run user provided handler when client is connected to server
        self.handler.on_connection(identifier, &socket)?;

        self.register_peer(Operation::Add, socket_fd, PeerRole::Client(identifier))?;
        self.next_client_id += 1;
        self.clients.insert(identifier, socket);
        Ok(())
    }

    fn handle_client_message(&mut self, client_id: u32) -> Result<()> {
        // read event -> broadcast to all client
        let mut accumulated_data = Vec::new();
        if let Some(client_soc) = self.clients.get_mut(&client_id) {
            let mut buffer = vec![0u8; 4096];
            loop {
                match client_soc.read(&mut buffer) {
                    Ok(0) => {
                        debug!("Read 0 bytes - connection closed or no more data");
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
        // now we need to provide this to user
        // and user will provide us the response to write to the socket
        match self.handler.on_message(client_id, &accumulated_data)? {
            HandlerAction::Broadcast(data) => {
                for (id, stream) in self
                    .clients
                    .iter_mut()
                    .skip_while(|(k, _)| **k == client_id)
                {
                    match stream.write(&data) {
                        Ok(size) => {
                            debug!("{} bytes sent from client {} to {}", size, client_id, id);
                        }
                        Err(_) => {
                            error!("failed to send data to client: {}", id);
                        }
                    }
                }
            }
            HandlerAction::Reply(data) => {
                if let Some(stream) = self.clients.get_mut(&client_id) {
                    match stream.write(&data) {
                        Ok(size) => {
                            debug!("{} bytes sent to client {}", size, client_id);
                            let _ = stream.flush();
                        }
                        Err(_) => {
                            error!("failed to send data to client: {}", client_id);
                        }
                    }
                }
            }
            HandlerAction::SendTo {
                target_client_id,
                data,
            } => {
                if let Some(stream) = self.clients.get_mut(&target_client_id) {
                    match stream.write(&data) {
                        Ok(size) => {
                            debug!("{} bytes sent to client {}", size, target_client_id);
                        }
                        Err(_) => {
                            error!("failed to send data to client: {}", target_client_id);
                        }
                    }
                }
            }
            HandlerAction::SendToAll(data) => {
                for (id, stream) in self.clients.iter_mut() {
                    match stream.write(&data) {
                        Ok(size) => {
                            debug!("{} bytes sent from client {} to {}", size, client_id, id);
                        }
                        Err(_) => {
                            error!("failed to send data to client: {}", id);
                        }
                    }
                }
            }
            HandlerAction::None => (),
        }

        Ok(())
    }

    fn handle_client_disconnection(&mut self, client_id: u32) -> Result<()> {
        if let Some(client_socket) = self.clients.remove(&client_id) {
            let fd = client_socket.as_raw_fd();
            self.deregister_client(Operation::Del, fd, client_id)?;

            self.handler.on_disconnect(client_id)?;
            info!(
                "client {} with address {:?} disconnected",
                client_id,
                client_socket.local_addr().unwrap()
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

    fn register_peer(&self, op: Operation, fd: i32, peer_role: PeerRole) -> Result<()> {
        let mut event = Event::new(peer_role).edge_trigerred().notify_read();

        if let PeerRole::Client(_) = &peer_role {
            event = event.notify_conn_close();
        }

        debug!(
            "Registering peer [{:?}] with fd {} and event flags {:#x}",
            peer_role,
            fd,
            event.event_type() as u32
        );
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
