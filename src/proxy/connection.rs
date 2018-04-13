use tokio::io;
use tokio::prelude::*;

// Needed since https://github.com/tokio-rs/tokio/commit/a6b307cfbefb568bd79eaf1d91edf9ab52d18533#diff-b4aea3e418ccdb71239b96952d9cddb6
// is not released yet.
use tokio_io::io::{ReadHalf,WriteHalf};

use std::mem;
use std;
use std::cmp::min;

use super::protocol::Preamble;

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
                    headers: lines.map(|l| { String::from(l) }).collect(),
                },
                buffered: self.buffer[preamble_end+4..self.position].to_vec(),
            },
            mem::replace(&mut self.io, None).unwrap(),
        )));
    }
}

struct RingBuffer<T> {
    buf: Vec<T>,
    start: usize,
    length: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(buf: Vec<T>) -> Self {
        RingBuffer { buf, start: 0, length: 0 }
    }
    pub fn with_next_writeable_chunk<F>(&mut self, f: F) -> Result<(),std::io::Error>
        where F: FnOnce(&mut [T]) -> Result<usize,std::io::Error>
    {
        if self.length < self.buf.len() {
            let begin_chunk = (self.start + self.length) % self.buf.len();
            let end_chunk = if self.start > begin_chunk {
                self.start
            } else {
                self.buf.len()
            };
            if begin_chunk < end_chunk {
                let written = f(&mut self.buf[begin_chunk..end_chunk])?;
                self.length += written;
                if self.length > self.buf.len() {
                    panic!("Producing more than the entire ring buffer");
                }
            }
        }
        Ok(())
    }
    pub fn with_next_readable_chunk<F>(&mut self, f: F) -> Result<(),std::io::Error>
        where F: FnOnce(&[T]) -> Result<usize,std::io::Error>
    {
        let end_chunk = min(self.start + self.length, self.buf.len());
        if end_chunk > self.start {
            let read = f(&self.buf[self.start..end_chunk])?;
            if read > self.length {
                panic!("Consuming more than the entire ring buffer");
            }
            self.start = (self.start + read) % self.buf.len();
            self.length -= read;
        }
        Ok(())
    }
}

const RINGBUFFER_SIZE : usize = 64*1024;

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
            self.t_to_s_buf.with_next_writeable_chunk(|next_writeable_chunk| {
                match t.poll_read(next_writeable_chunk) {
                    Ok(Async::Ready(0)) => {
                        t.shutdown();
                        s.shutdown();
                        Err(zero_op())
                    }
                    Ok(Async::Ready(n)) => {
                        none_are_ready = false;
                        Ok(n)
                    }
                    Ok(Async::NotReady) => {
                        Ok(0)
                    }
                    Err(err) => {
                        t.shutdown();
                        s.shutdown();
                        Err(err)
                    }
                }
            })?;
            self.t_to_s_buf.with_next_readable_chunk(|next_readable_chunk| {
                match s.poll_write(next_readable_chunk) {
                    Ok(Async::Ready(0)) => {
                        t.shutdown();
                        s.shutdown();
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
                        t.shutdown();
                        s.shutdown();
                        Err(err)
                    }
                }
            })?;
            self.s_to_t_buf.with_next_writeable_chunk(|next_writeable_chunk| {
                match s.poll_read(next_writeable_chunk) {
                    Ok(Async::Ready(0)) => {
                        t.shutdown();
                        s.shutdown();
                        Err(zero_op())
                    }
                    Ok(Async::Ready(n)) => {
                        none_are_ready = false;
                        Ok(n)
                    }
                    Ok(Async::NotReady) => Ok(0),
                    Err(err) => {
                        t.shutdown();
                        s.shutdown();
                        Err(err)
                    }
                }
            })?;
            self.s_to_t_buf.with_next_readable_chunk(|next_readable_chunk| {
                match t.poll_write(next_readable_chunk) {
                    Ok(Async::Ready(0)) => {
                        t.shutdown();
                        s.shutdown();
                        Err(zero_op())
                    }
                    Ok(Async::Ready(n)) => {
                        none_are_ready = false;
                        Ok(n)
                    }
                    Ok(Async::NotReady) => Ok(0),
                    Err(err) => {
                        t.shutdown();
                        s.shutdown();
                        Err(err)
                    }
                }
            })?;
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
