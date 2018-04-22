use tokio::io;
use tokio::prelude::*;

use std::mem;
use std;

use super::protocol::Preamble;
use ::ringbuffer::RingBuffer;

#[derive(Debug)]
pub struct Incoming<IO : AsyncRead> {
    io: Option<IO>,
    buffer: Vec<u8>,
    position: usize,
}

#[derive(Debug)]
pub struct IncomingResult {
    pub preamble: Preamble,
    pub buffered: Vec<u8>,
}

impl<IO: AsyncRead> Incoming<IO> {
    pub fn new(io: IO) -> Incoming<IO> {
        Incoming {
            io: Some(io),
            buffer: vec![0; 1024],
            position: 0,
        }
    }
}

fn findslice(needle: &[u8], haystack: &[u8]) -> Option<usize> {
    let n = needle.len();
    let k = haystack.len();
    if n > k {
        return None;
    }
    for (ix,window) in haystack.windows(n).enumerate() {
        if window == needle {
            return Some(ix);
        }
    }
    return None;
}

impl<IO: AsyncRead> Future for Incoming<IO> {
    type Item = (IncomingResult, IO);
    type Error = io::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        let preamble_end : usize;
        loop {
            if let Some(ref mut io) = self.io {
                let n = try_ready!(io.poll_read(&mut self.buffer[self.position..]));
                self.position += n;
                // TODO: error out on zero
                match findslice(b"\r\n\r\n", &self.buffer[..self.position]) {
                    Some(ix) => {
                        preamble_end = ix;
                        break;
                    }
                    None => return Ok(Async::NotReady),
                }
            } else {
                panic!("Polling resolved future");
            }
        }
        let preamble = String::from_utf8(self.buffer[..preamble_end].to_vec()).expect("invalid data received");
        let mut lines = preamble.lines();
        let first_line = lines.next().unwrap();

        let mut items = first_line.split(" ");
        let method = String::from(items.next().unwrap());
        let uri = String::from(items.next().unwrap());
        let http_version = String::from(items.next().unwrap());

        return Ok(Async::Ready((
            IncomingResult {
                preamble: Preamble {
                    method,
                    uri,
                    http_version,
                    headers: lines.map(String::from).collect(),
                },
                buffered: self.buffer[preamble_end+4..self.position].to_vec(),
            },
            mem::replace(&mut self.io, None).unwrap(),
        )));
    }
}

const RINGBUFFER_SIZE: usize = 64*1024;

pub struct TwoWayPipe<T: AsyncRead+AsyncWrite,S: AsyncRead+AsyncWrite> {
    t: T,
    s: S,
    t_to_s_buf: RingBuffer<u8>,
    s_to_t_buf: RingBuffer<u8>,
}

impl<T: AsyncRead+AsyncWrite,S: AsyncRead+AsyncWrite> TwoWayPipe<T, S> {
    pub fn new(t: T, s: S) -> Self {
        TwoWayPipe {
            t, s,
            t_to_s_buf: RingBuffer::new([0u8; RINGBUFFER_SIZE].to_vec()),
            s_to_t_buf: RingBuffer::new([0u8; RINGBUFFER_SIZE].to_vec()),
        }
    }
}

fn zero_op() -> io::Error {
    io::Error::new(io::ErrorKind::WriteZero, "zero-length operation")
}

impl<T: AsyncRead+AsyncWrite,S: AsyncRead+AsyncWrite> Future for TwoWayPipe<T, S> {
    type Item = ();
    type Error = std::io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let s = &mut self.s;
        let t = &mut self.t;
        loop {
            let mut none_are_ready = true;

            let mut t_readable = true;
            self.t_to_s_buf.with_next_writeable_chunk(|next_writeable_chunk| {
                match t.poll_read(next_writeable_chunk) {
                    Ok(Async::Ready(0)) => {
                        t_readable = false;
                        Ok(0)
                    }
                    Ok(Async::Ready(n)) => {
                        none_are_ready = false;
                        Ok(n)
                    }
                    Ok(Async::NotReady) => {
                        Ok(0)
                    }
                    Err(err) => {
                        t.shutdown().ok();
                        s.shutdown().ok();
                        Err(err)
                    }
                }
            })?;
            self.t_to_s_buf.with_next_readable_chunk(|next_readable_chunk| {
                match s.poll_write(next_readable_chunk) {
                    Ok(Async::Ready(0)) => {
                        t.shutdown().ok();
                        s.shutdown().ok();
                        Err(zero_op())
                    }
                    Ok(Async::Ready(n)) => {
                        none_are_ready = false;
                        Ok(n)
                    }
                    Ok(Async::NotReady) => {
                        Ok(0)
                    },
                    Err(err) => {
                        t.shutdown().ok();
                        s.shutdown().ok();
                        Err(err)
                    }
                }
            })?;
            let mut s_readable = true;
            self.s_to_t_buf.with_next_writeable_chunk(|next_writeable_chunk| {
                match s.poll_read(next_writeable_chunk) {
                    Ok(Async::Ready(0)) => {
                        s_readable = false;
                        Ok(0)
                    }
                    Ok(Async::Ready(n)) => {
                        none_are_ready = false;
                        Ok(n)
                    }
                    Ok(Async::NotReady) => Ok(0),
                    Err(err) => {
                        t.shutdown().ok();
                        s.shutdown().ok();
                        Err(err)
                    }
                }
            })?;
            self.s_to_t_buf.with_next_readable_chunk(|next_readable_chunk| {
                match t.poll_write(next_readable_chunk) {
                    Ok(Async::Ready(0)) => {
                        t.shutdown().ok();
                        s.shutdown().ok();
                        Err(zero_op())
                    }
                    Ok(Async::Ready(n)) => {
                        none_are_ready = false;
                        Ok(n)
                    }
                    Ok(Async::NotReady) => Ok(0),
                    Err(err) => {
                        t.shutdown().ok();
                        s.shutdown().ok();
                        Err(err)
                    }
                }
            })?;
            if !s_readable && self.s_to_t_buf.len() == 0 {
                return Ok(Async::Ready(()))
            }
            if !t_readable && self.t_to_s_buf.len() == 0 {
                return Ok(Async::Ready(()))
            }
            if none_are_ready {
                return Ok(Async::NotReady)
            }
        }
    }
}

pub fn two_way_pipe<T,S>(t:T, s:S) -> Box<Future<Item=(),Error=std::io::Error>+Send>
where
    T: AsyncRead+AsyncWrite+Send+'static,
    S: AsyncRead+AsyncWrite+Send+'static {

    let pipe = TwoWayPipe::new(t,s);
    Box::new(pipe)
}
