use std::mem;
use tokio::io;
use tokio::prelude::*;
use uri::Uri;

#[derive(Debug)]
pub struct Preamble {
    pub method: String,
    pub uri: String,
    pub http_version: String,
    pub headers: Vec<String>,
}

#[derive(Debug)]
enum State {
    WritingMethod {pos: usize},
    WritingSpace1 {},
    WritingUri {pos: usize},
    WritingSpace2 {},
    WritingHTTPVersion {pos: usize},
    //WritingHeaders {header: usize},
    Done,
}

#[derive(Debug)]
pub struct WritePreamble<IO: io::AsyncWrite> {
    preamble: Option<Preamble>,
    state: State,
    io: IO,
}

impl<IO: io::AsyncWrite> Future for WritePreamble<IO> {
    type Item = Preamble;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if let Some(ref mut preamble) = self.preamble {
                match self.state {
                    State::WritingMethod { ref mut pos } => {
                        while *pos < preamble.method.len() {
                            let n = try_ready!(self.io.poll_write(preamble.method[*pos..].as_bytes()));
                            *pos += n;
                        }
                    }
                    State::WritingSpace1 { } | State::WritingSpace2 { } => {
                        try_ready!(self.io.poll_write(b" "));
                    }
                    State::WritingUri { ref mut pos } => {
                        while *pos < preamble.uri.len() {
                            let n = try_ready!(self.io.poll_write(preamble.uri[*pos..].as_bytes()));
                            *pos += n;
                        }
                    }
                    State::WritingHTTPVersion { ref mut pos } => {
                        while *pos < preamble.http_version.len() {
                            let n = try_ready!(self.io.poll_write(preamble.http_version[*pos..].as_bytes()));
                            *pos += n;
                        }
                    }
                    // TODO
                    State::Done => (),
                }
            }
            self.state = match self.state {
                State::WritingMethod { .. } => State::WritingSpace1 { },
                State::WritingSpace1 { }    => State::WritingUri { pos: 0 },
                State::WritingUri { .. }    => State::WritingSpace2 { },
                State::WritingSpace2 { }    => State::WritingHTTPVersion { pos: 0 },
                State::WritingHTTPVersion { .. } => State::Done,
                State::Done                 => return Ok(Async::Ready(mem::replace(&mut self.preamble, None).unwrap())),
            };
        }
    }
}

impl Preamble {
    pub fn write<IO: io::AsyncWrite>(self, io: IO) -> WritePreamble<IO> {
        WritePreamble { preamble: Some(self), state: State::WritingMethod { pos: 0 }, io }
    }
}
