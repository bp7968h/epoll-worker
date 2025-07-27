use std::{
    io::{Error, Result},
    os::fd::RawFd,
};

use log::{debug, error};

use crate::ep_syscall;

/// Represents either server or client
///
/// This helps us to register to epoll
/// and also to identify whose events we are operating on
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum PeerRole {
    Server,
    Client(u64),
}

impl From<u64> for PeerRole {
    fn from(value: u64) -> Self {
        match value {
            0 => PeerRole::Server,
            others => PeerRole::Client(others),
        }
    }
}

impl From<PeerRole> for u64 {
    fn from(value: PeerRole) -> Self {
        match value {
            PeerRole::Server => 0,
            PeerRole::Client(id) => id,
        }
    }
}

/// Performable Operations for Target fd
///
/// These are all the valid values for `op` argument of `epoll_ctl`
/// Where;
///     ADD = EPOLL_CTL_ADD
///     DEL = EPOLL_CTL_DEL
///     MOD = EPOLL_CTL_MOD
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Operation {
    /// Add entry to the interest list of the epoll instance
    Add,
    /// Remove the target file descriptor from the interest list
    Del,
    /// Change the settings associated with fd in the interest list
    /// to the new settings specified in event
    Mod,
}

impl From<Operation> for i32 {
    fn from(value: Operation) -> Self {
        match value {
            Operation::Add => 1,
            Operation::Del => 2,
            Operation::Mod => 3,
        }
    }
}

/// Avalilable event types and input for `Event`
///
/// These variants are used to bitmask the `event` filed in `Event`
#[allow(dead_code)]
#[repr(i32)]
pub(crate) enum EventType {
    /// Read operation
    Epollin = 0x1,
    /// Write operation
    Epollout = 0x4,
    /// Stream socket peer closed connection or shut down
    Epollrdhup = 0x2000,
    /// Exceptional condition
    Epollpri = 0x2,
    /// Error condition
    Epollerr = 0x8,
    /// Hang up happened on associated fd
    Epollhup = 0x10,
    /// Request edge-trigerred notification
    Epollet = 1 << 31,
    /// Request one-shot notification
    Epolloneshot = 1 << 30,
}

/// Corresponds to Linux's `epoll_event`
///
/// events - is the bit mask composed by ORing together zero or more event
///
/// data means user data/identifier
#[derive(Debug)]
#[repr(C, packed(1))]
pub(crate) struct Event {
    /// bit mask composed by ORing together zero or more event types
    /// returned by `epoll_wait`, and input flags which affect its behaviour
    events: u32,
    /// Data kernel saves and returns via `epoll_wait` when
    /// file descriptor becomes ready
    data: u64,
}

#[allow(dead_code)]
impl Event {
    pub fn new(bitmask: u32, identifier: PeerRole) -> Self {
        Event {
            events: bitmask,
            data: identifier.into(),
        }
    }

    pub fn event_type(&self) -> u32 {
        self.events
    }

    pub fn role(&self) -> PeerRole {
        PeerRole::from(self.data)
    }

    pub fn data(&self) -> u64 {
        self.data
    }
}

/// Epoll wrapper
///
/// Encapsulates epoll operations including
/// creating epoll instance
/// getting ready events
/// adding interest to epoll instance,
/// modifyinf interest to epoll instance,
/// deleting insterest from epoll instance
pub(crate) struct Epoll {
    epfd: RawFd,
}

impl Epoll {
    /// Create new instance of epoll
    pub fn new() -> Result<Self> {
        let epfd = ep_syscall!(epoll_create1(0))?;

        // Validate the file descriptor (F_GETFD)
        if let Err(e) = ep_syscall!(fcntl(epfd, 1)) {
            let _ = ep_syscall!(close(epfd));
            return Err(e);
        }

        Ok(Epoll { epfd })
    }

    /// Get events from ready list
    pub fn wait(&self, events: &mut Vec<Event>, timeout: Option<i32>) -> Result<()> {
        let max_events = events.capacity() as i32;
        let timeout = timeout.unwrap_or(1000);

        let res = ep_syscall!(epoll_wait(
            self.epfd,
            events.as_mut_ptr(),
            max_events,
            timeout
        ))?;

        // Kernel should always return the bounded number of events
        if res > max_events {
            // EINVAL = 22 (invalid argument)
            return Err(Error::from_raw_os_error(22));
        }
        unsafe {
            events.set_len(res as usize);
        }

        if timeout.is_negative() {
            debug!("Epoll polling timeout reached, retrying...");
        } else {
            debug!("Received {} events from epoll", res);
        }
        Ok(())
    }

    /// Add event to interest list
    pub fn add_interest(&self, fd: RawFd, mut event: Event) -> Result<()> {
        self.control_interest(Operation::Add, fd, Some(&mut event))
    }

    /// Modify event in interest list
    pub fn modify_interest(&self, fd: RawFd, mut event: Event) -> Result<()> {
        self.control_interest(Operation::Mod, fd, Some(&mut event))
    }

    /// Remove event from interest list
    pub fn remove_interest(&self, fd: RawFd) -> Result<()> {
        self.control_interest(Operation::Del, fd, None)?;

        if let Err(e) = ep_syscall!(close(fd)) {
            error!("Failed to close epoll fd {}: {}", fd, e);
        }
        Ok(())
    }

    fn control_interest(&self, op: Operation, fd: RawFd, event: Option<&mut Event>) -> Result<()> {
        if fd < 0 {
            // EBADF = 9 (Bad file descriptor)
            return Err(Error::from_raw_os_error(9));
        }

        let event_ptr = match event {
            Some(event) => event as *mut Event,
            None => std::ptr::null_mut(),
        };

        let _ = ep_syscall!(epoll_ctl(self.epfd, i32::from(op), fd, event_ptr))?;

        Ok(())
    }

    pub fn fd(&self) -> RawFd {
        self.epfd
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        if let Err(e) = ep_syscall!(close(self.epfd)) {
            error!("Failed to close epoll fd {}: {}", self.epfd, e);
        }
    }
}
