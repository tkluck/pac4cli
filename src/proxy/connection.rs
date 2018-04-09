extern crate tokio;
use self::tokio::io;
use self::tokio::prelude::*;

use std::str;

#[derive(Debug)]
pub struct Incoming<IO : AsyncRead> {
    io: IO,
    buffer: Vec<u8>,
    position: usize,
    streamable: bool,
}

#[derive(Debug)]
pub struct IncomingResult {
    // I spent ample time trying to get &'a [u8] to work for all of these,
    // indexing into the buffer that's held by Incoming. I couldn't get
    // that to compile.
    method: Vec<u8>,
    uri: Vec<u8>,
    http_version: Vec<u8>,
    headers: Vec<Vec<u8>>,
    buffer: Vec<u8>,
    //io: IO,
}

impl<IO: AsyncRead> Incoming<IO> {
    pub fn new(io: IO) -> Incoming<IO> {
        Incoming {
            io,
            buffer: vec![0; 1024],
            position: 0,
            streamable: false,
        }
    }
}

fn findslice(needle: &[u8], haystack: &[u8]) -> Option<usize> {
    let n = needle.len();
    let k = haystack.len();
    if n > k {
        return None;
    }
    for ix in 0..haystack.len() - n + 1 {
        if &haystack[ix..ix+n] == needle {
            return Some(ix);
        }
    }
    return None;
}

impl<IO: AsyncRead> Future for Incoming<IO> {
    type Item = IncomingResult;
    type Error = io::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        match self.io.poll_read(&mut self.buffer[self.position..])? {
            Async::Ready(n) => {
                let prev_n = self.position;
                self.position += n;
                match findslice(b"\r\n\r\n", &self.buffer[..self.position]) {
                    Some(ix) => {
                        let preamble_end = ix;
                        let preamble = str::from_utf8(&self.buffer[..preamble_end]).expect("invalid data received");
                        let mut lines = preamble.lines();
                        let first_line = lines.next().unwrap();

                        let mut items = first_line.split(" ");
                        let method = items.next().unwrap().as_bytes().to_vec();
                        let uri = items.next().unwrap().as_bytes().to_vec();
                        let http_version = items.next().unwrap().as_bytes().to_vec();

                        Ok(Async::Ready(IncomingResult {
                            method,
                            uri,
                            http_version,
                            headers: lines.map(|l| { l.as_bytes().to_vec() }).collect(),
                            buffer: self.buffer[preamble_end+4..self.position].to_vec(),
                            //io: self.io,
                        }))
                    }
                    None => Ok(Async::NotReady)
                }
            }
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}
