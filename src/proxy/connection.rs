use tokio::io;
use tokio::net::TcpStream;
use tokio::prelude::*;

use crate::proxy::protocol::Preamble;

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
    for (ix,window) in haystack.windows(n).enumerate() {
        if window == needle {
            return Some(ix);
        }
    }
    return None;
}

pub async fn sniff_incoming_connection(io: &mut TcpStream) -> io::Result<IncomingResult> {
    let mut position = 0;
    let preamble_end : usize;
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
    let preamble = String::from_utf8(buffer[..preamble_end].to_vec()).expect("invalid data received");
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
        buffered: buffer[preamble_end+4..position].to_vec(),
    });
}

pub async fn two_way_pipe(t: &mut TcpStream, s: &mut TcpStream) -> io::Result<()> {

    let (mut tr, mut tw) = t.split();
    let (mut sr, mut sw) = s.split();

    let s_to_t = tokio::io::copy(&mut sr, &mut tw);
    let t_to_s = tokio::io::copy(&mut tr, &mut sw);

    tokio::join!(s_to_t, t_to_s);

    Ok(())
}
