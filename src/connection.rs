use tokio::io;
use tokio::net;
use tokio::prelude::*;

#[derive(Debug, Clone)]
pub struct Preamble {
    pub method: String,
    pub uri: String,
    pub http_version: String,
    pub headers: Vec<String>,
}

impl Preamble {
    pub async fn write<IO: std::marker::Unpin + io::AsyncWrite>(
        self,
        io: &mut IO,
    ) -> io::Result<()> {
        io.write_all(self.method.as_bytes()).await?;
        io.write_all(b" ").await?;
        io.write_all(self.uri.as_bytes()).await?;
        io.write_all(b" ").await?;
        io.write_all(self.http_version.as_bytes()).await?;
        io.write_all(b"\r\n").await?;
        for header in self.headers {
            io.write_all(header.as_bytes()).await?;
            io.write_all(b"\r\n").await?;
        }
        io.write_all(b"\r\n").await?;
        return Ok(());
    }
}

#[derive(Debug)]
pub struct IncomingResult {
    pub preamble: Preamble,
    pub buffered: Vec<u8>,
}

fn findslice(needle: &[u8], haystack: &[u8]) -> Option<usize> {
    let n = needle.len();
    let k = haystack.len();
    if n > k {
        return None;
    }
    for (ix, window) in haystack.windows(n).enumerate() {
        if window == needle {
            return Some(ix);
        }
    }
    return None;
}

pub async fn sniff_incoming_connection(io: &mut net::TcpStream) -> io::Result<IncomingResult> {
    let mut position = 0;
    let preamble_end: usize;
    let mut buffer = vec![0; 1024];
    loop {
        let n = io.read(&mut buffer[position..]).await?;
        position += n;
        // TODO: error out on zero
        if let Some(ix) = findslice(b"\r\n\r\n", &buffer[..position]) {
            preamble_end = ix;
            break;
        }
    }
    let preamble =
        String::from_utf8(buffer[..preamble_end].to_vec()).expect("invalid data received");
    let mut lines = preamble.lines();
    let first_line = lines.next().unwrap();

    let mut items = first_line.split(" ");
    let method = String::from(items.next().unwrap());
    let uri = String::from(items.next().unwrap());
    let http_version = String::from(items.next().unwrap());

    return Ok(IncomingResult {
        preamble: Preamble {
            method,
            uri,
            http_version,
            headers: lines.map(String::from).collect(),
        },
        buffered: buffer[preamble_end + 4..position].to_vec(),
    });
}

pub async fn two_way_pipe(t: &mut net::TcpStream, s: &mut net::TcpStream) -> io::Result<()> {
    let (mut tr, mut tw) = t.split();
    let (mut sr, mut sw) = s.split();

    let s_to_t = tokio::io::copy(&mut sr, &mut tw);
    let t_to_s = tokio::io::copy(&mut tr, &mut sw);

    tokio::join!(s_to_t, t_to_s);

    Ok(())
}
