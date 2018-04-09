use tokio::io;
use tokio::prelude::*;

use std::str;
use std::mem;
use std::ops::Range;

use super::protocol::Preamble;

#[derive(Debug)]
pub struct Incoming<IO : AsyncRead> {
    io: IO,
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
            io,
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
    type Item = IncomingResult;
    type Error = io::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        loop {
            match self.io.poll_read(&mut self.buffer[self.position..]) {
                Ok(Async::Ready(n)) => {
                    self.position += n;
                    match findslice(b"\r\n\r\n", &self.buffer[..self.position]) {
                        Some(preamble_end) => {
                            let preamble = String::from_utf8(self.buffer[..preamble_end].to_vec()).expect("invalid data received");
                            let mut lines = preamble.lines();
                            let first_line = lines.next().unwrap();

                            let mut items = first_line.split(" ");
                            let method = String::from(items.next().unwrap());
                            let uri = String::from(items.next().unwrap());
                            let http_version = String::from(items.next().unwrap());

                            return Ok(Async::Ready(IncomingResult {
                                preamble: Preamble {
                                    method,
                                    uri,
                                    http_version,
                                    headers: Vec::new(),
                                },
                                buffered: self.buffer[preamble_end+4..self.position].to_vec(),
                            }))
                        }
                        None => return Ok(Async::NotReady),
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(err) => return Err(err),
            }
        }
    }
}

pub fn two_way_pipe<T,S>(t:T, s:S) -> future::Join<io::Copy<T,S>,io::Copy<S,T>>
where
    T: AsyncRead+AsyncWrite+Copy,
    S: AsyncRead+AsyncWrite+Copy {
    return io::copy(t,s).join(io::copy(s,t));
}
