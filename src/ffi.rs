//! Epoll foreign function

use crate::Event;
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
    pub fn epoll_create1(size: i32) -> i32;

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

    /// Performs operation on open file descriptor
    ///
    /// Operation is defined by `op` argument.
    /// Operations can be different type, but we are
    /// only interested in verifying if the file descriptor
    /// is valid or not.
    ///
    ///     F_GETFD - returns the file descriptor flags
    ///               value of F_GETFD is 1
    pub fn fcntl(fd: i32, op: i32, ...) -> i32;
}
