use std::{
    collections::VecDeque,
    io::{ErrorKind, Result, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    os::fd::{AsRawFd, RawFd},
};

#[derive(Debug)]
pub struct ClientState {
    stream: TcpStream,
    read_buffer: Vec<u8>,
    write_queue: VecDeque<Vec<u8>>,
    write_buffer: Option<Vec<u8>>,
    write_offset: usize,
    current_interests: u32,
}

impl ClientState {
    pub fn new(stream: TcpStream) -> Self {
        ClientState {
            stream,
            read_buffer: Vec::with_capacity(16384),
            write_queue: VecDeque::with_capacity(16),
            write_buffer: None,
            write_offset: 0,
            current_interests: 0,
        }
    }

    pub fn queue_write(&mut self, data: Vec<u8>) {
        self.write_queue.push_back(data);
    }

    pub fn has_pending_writes(&self) -> bool {
        !self.write_queue.is_empty() || self.write_buffer.is_some()
    }

    pub fn flush_writes(&mut self) -> Result<bool> {
        loop {
            if self.write_buffer.is_none() {
                if let Some(next_buffer) = self.write_queue.pop_front() {
                    self.write_buffer = Some(next_buffer);
                    self.write_offset = 0;
                } else {
                    self.stream.shutdown(Shutdown::Both)?;
                    return Ok(true);
                }
            }

            if let Some(ref buffer) = self.write_buffer {
                match self.stream.write(&buffer[self.write_offset..]) {
                    Ok(0) => {
                        // Cannot Write, Connection closed
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::BrokenPipe,
                            "Connection closed",
                        ));
                    }
                    Ok(bytes_written) => {
                        self.write_offset += bytes_written;

                        if self.write_offset >= buffer.len() {
                            self.write_buffer = None;
                            self.write_offset = 0;
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        // CAnnot write more now
                        return Ok(false);
                    }
                    Err(e) => return Err(e),
                }
            }
        }
    }

    pub fn current_interests(&self) -> u32 {
        self.current_interests
    }

    pub fn set_current_interests(&mut self, interests: u32) {
        self.current_interests = interests;
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
