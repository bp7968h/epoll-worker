unsafe extern "C" {
    /// Creates new epoll instance
    ///
    /// # Arguments
    ///
    /// * `size` - Size is required but ingnored and must be greater than 0
    ///
    /// # Returns
    ///
    /// The file descriptor of the epoll instance or `-1` if there is any error
    /// and the error is set to `errno` which is basically the `last_os_error`
    pub fn epoll_create(size: i32) -> i32;

    /// Closes a file descriptor
    ///
    /// This is used to close the epoll instance when no longer needed.
    /// OS frees the resources associated with the epoll instance that we created.
    ///
    /// # Returns
    ///
    /// `0` on success and `-1` on error
    pub fn close(fd: i32) -> i32;

    /// Add, modify or remove entries in interest list of epoll instance
    ///
    /// # Arguments
    ///
    /// * `epfd` - epoll instance file descriptor
    /// * `op` - operation to be performed for target file descriptor
    /// * `fd` - target file descriptor
    /// * `event` -
    pub fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *mut Event) -> i32;

    /// Wait for events on epoll instance
    ///
    /// # Arguments
    ///
    /// * `epfd` - epoll instance file descriptor
    /// * `events` - buffer to fill the returned events notification
    /// * `max_events` - number of max events to be filled, must be greater than zero
    /// * `timeouot` - number of milliseconds that `epoll_wait` will block
    pub fn epoll_wait(epfd: i32, events: *mut Event, max_events: i32, timeout: i32) -> i32;
}

/// Represents either server or client
///
/// This helps us to register to epoll
/// and also to identify whose events we are operating on
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum PeerRole {
    Server,
    Client(u32),
}

impl From<PeerRole> for u32 {
    fn from(value: PeerRole) -> Self {
        match value {
            PeerRole::Server => 0,
            PeerRole::Client(id) => id,
        }
    }
}

/// Corresponds to Linux's `epoll_event`
///
/// events - is the bit mask composed by ORing together zero or more event
///
/// data means user data/identifier
#[repr(C)]
pub struct Event {
    /// bit mask composed by ORing together zero or more event types
    /// returned by `epoll_wait`, and input flags which affect its behaviour
    events: u32,
    /// Data kernel saves and returns via `epoll_wait` when
    /// file descriptor becomes ready
    data: u32,
}

/// Performable Operations for Target fd
///
/// These are all the valid values for `op` argument of `epoll_ctl`
///
/// Add entry to the interest list of the epoll instance
pub const EPOLL_CTL_ADD: i32 = 1;
/// Remove the target file descriptor from the interest list
pub const EPOLL_CTL_DEL: i32 = 2;
/// Change the settings associated with fd in the interest list
/// to the new settings specified in event
// pub const EPOLL_CTL_MOD: i32 = 3;

/// Avaiable event types for Event
///
/// File read operations
pub const EPOLLIN: i32 = 0x1;
/// File write operations
// pub const EPOLLOUT: i32 = 0x4;
/// Stream socket peer closed connection
pub const EPOLLRDHUP: i32 = 0x2000;

/// Available input flags
///
/// Request edge-trigerred notification
pub const EPOLLET: i32 = 1 << 31;

impl Event {
    pub fn new(identifier: PeerRole) -> Self {
        Event {
            events: 0,
            data: identifier.into(),
        }
    }

    pub fn event_type(&self) -> u32 {
        self.events
    }

    pub fn data(&self) -> u32 {
        self.data
    }

    pub fn edge_trigerred(self) -> Self {
        Event {
            events: self.events | EPOLLET as u32,
            ..self
        }
    }

    pub fn notify_read(self) -> Self {
        Event {
            events: self.events | EPOLLIN as u32,
            ..self
        }
    }

    // pub fn notify_write(self) -> Self {
    //     Event {
    //         events: self.events | EPOLLOUT as u32,
    //         ..self
    //     }
    // }

    pub fn notify_conn_close(self) -> Self {
        Event {
            events: self.events | EPOLLRDHUP as u32,
            ..self
        }
    }
}
