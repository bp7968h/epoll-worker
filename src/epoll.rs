use std::ffi::c_int;

unsafe extern "C" {
    pub fn epoll_create(size: c_int) -> i32;
}
