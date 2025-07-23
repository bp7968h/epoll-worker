use std::{
    io::{Result, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    os::fd::{AsRawFd, RawFd},
};

#[derive(Debug)]
pub struct ClientState {
    stream: TcpStream,
    read_buffer: Vec<u8>,
}

impl ClientState {
    pub fn new(stream: TcpStream) -> Self {
        ClientState {
            stream,
            read_buffer: Vec::new(),
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.stream.write(buf)
    }

    pub fn stream_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }

    pub fn read_buf_mut(&mut self) -> &mut Vec<u8> {
        &mut self.read_buffer
    }

    pub fn read_buf(&self) -> &[u8] {
        &self.read_buffer
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.stream.local_addr()
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.stream.shutdown(Shutdown::Both)
    }
}
