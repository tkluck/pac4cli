use std::mem;
use tokio::io;
use tokio::prelude::*;

#[derive(Debug,Clone)]
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
    WritingHeader {header: usize, pos: usize},
    WritingTerminator {pos: usize},
    Done,
}

#[derive(Debug)]
pub struct WritePreamble<IO: io::AsyncWrite> {
    preamble: Option<Preamble>,
    state: State,
    io: Option<IO>,
}

impl<IO: io::AsyncWrite> Future for WritePreamble<IO> {
    type Item = (Preamble,IO);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if let Some(ref mut preamble) = self.preamble {
                if let Some(ref mut io) = self.io {
                    match self.state {
                        State::WritingMethod { ref mut pos } => {
                            while *pos < preamble.method.len() {
                                let n = try_ready!(io.poll_write(preamble.method[*pos..].as_bytes()));
                                *pos += n;
                            }
                        }
                        State::WritingSpace1 { } | State::WritingSpace2 { } => {
                            try_ready!(io.poll_write(b" "));
                        }
                        State::WritingUri { ref mut pos } => {
                            while *pos < preamble.uri.len() {
                                let n = try_ready!(io.poll_write(preamble.uri[*pos..].as_bytes()));
                                *pos += n;
                            }
                        }
                        State::WritingHTTPVersion { ref mut pos } => {
                            while *pos < preamble.http_version.len() {
                                let n = try_ready!(io.poll_write(preamble.http_version[*pos..].as_bytes()));
                                *pos += n;
                            }
                        }
                        State::WritingHeader { header, ref mut pos } => {
                            while *pos < 2 {
                                let n = try_ready!(io.poll_write(&b"\r\n"[*pos..]));
                                *pos += n;
                            }
                            while header < preamble.headers.len() && *pos - 2 < preamble.http_version.len() {
                                let n = try_ready!(io.poll_write(preamble.headers[header][*pos-2..].as_bytes()));
                                *pos += n;
                            }
                        }
                        State::WritingTerminator { ref mut pos } => {
                            while *pos < 4 {
                                let n = try_ready!(io.poll_write(&b"\r\n\r\n"[*pos..]));
                                *pos += n;
                            }
                        }
                        State::Done => (),
                    }
                }
            }
            self.state = match self.state {
                State::WritingMethod { .. } => State::WritingSpace1 { },
                State::WritingSpace1 { }    => State::WritingUri { pos: 0 },
                State::WritingUri { .. }    => State::WritingSpace2 { },
                State::WritingSpace2 { }    => State::WritingHTTPVersion { pos: 0 },
                State::WritingHTTPVersion { .. } => State::WritingHeader { header: 0, pos: 0 },
                State::WritingHeader { header, .. } => {
                    if let Some(ref preamble) = self.preamble {
                        let new_header = header + 1;
                        if new_header < preamble.headers.len() {
                            State::WritingHeader { header: new_header, pos: 0 }
                        } else {
                            State::WritingTerminator { pos: 0 }
                        }
                    } else {
                        panic!("Polling resolved future");
                    }
                },
                State::WritingTerminator { ..} => State::Done,
                State::Done                 => return Ok(Async::Ready((mem::replace(&mut self.preamble, None).unwrap(), mem::replace(&mut self.io, None).unwrap()))),
            };
        }
    }
}

impl Preamble {
    pub fn write<IO: io::AsyncWrite>(self, io: IO) -> WritePreamble<IO> {
        WritePreamble { preamble: Some(self), state: State::WritingMethod { pos: 0 }, io: Some(io) }
    }
}
