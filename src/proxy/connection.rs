use tokio::io;
use tokio::prelude::*;

// Needed since https://github.com/tokio-rs/tokio/commit/a6b307cfbefb568bd79eaf1d91edf9ab52d18533#diff-b4aea3e418ccdb71239b96952d9cddb6
// is not released yet.
use tokio_io::io::{ReadHalf,WriteHalf};

use std::mem;

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

pub fn two_way_pipe<T,S>(t:T, s:S) -> future::Join<io::Copy<ReadHalf<T>,WriteHalf<S>>,io::Copy<ReadHalf<S>,WriteHalf<T>>>
where
    T: AsyncRead+AsyncWrite,
    S: AsyncRead+AsyncWrite {
    let (t_read, t_write) = t.split();
    let (s_read, s_write) = s.split();
    return io::copy(t_read,s_write).join(io::copy(s_read,t_write));
}
