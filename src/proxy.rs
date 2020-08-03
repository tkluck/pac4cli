use std::net::ToSocketAddrs;

use tokio;
use tokio::io;
use tokio::net;
use tokio::prelude::*;

use uri;

mod connection;
mod protocol;

use self::connection::two_way_pipe;
use crate::wpad;
use crate::wpad::ProxySuggestion;

async fn send_error(conn: &mut net::TcpStream) -> io::Result<()> {
    conn.write_all(b"<h1>Could not connect</h1>").await?;
    // TODO(tkluck): close it; conn.close();
    Ok(())
}

pub async fn process_socket<T>(
    mut downstream_connection: net::TcpStream,
    proxy_resolver: &wpad::ProxyResolver<T>,
) -> io::Result<()>
where
    T: wpad::NetworkEnvironment,
{
    let incoming_result = connection::sniff_incoming_connection(&mut downstream_connection).await?;

    // First, find a hostname + url for doing the pacparser lookup
    let (ref url, ref host, port) = if incoming_result.preamble.method == "CONNECT" {
        let mut parts = incoming_result.preamble.uri.split(":");
        let host = String::from(parts.next().expect("No host in connect spec"));
        let port = parts
            .next()
            .expect("No port in connect spec")
            .parse::<u16>()
            .expect("Invalid port in connect spec");
        (host.clone(), host, port)
    } else {
        let uri = uri::Uri::new(&incoming_result.preamble.uri).expect("Can't parse incoming uri");
        let host = uri.host.expect("No host in URI; aborting");
        let default_port: u16 = if uri.scheme == "https" { 443 } else { 80 };
        let port = uri.port.unwrap_or(default_port);
        (incoming_result.preamble.uri.clone(), host, port)
    };
    debug!("Destination is {}:{}", host, port);

    let upstream_address: (String, u16);
    let preamble_for_upstream: Option<protocol::Preamble>;
    let buffered_for_upstream: Vec<u8>;
    let my_response_for_downstream: Option<&'static [u8]>;

    match proxy_resolver.find_proxy(url, host) {
        ProxySuggestion::Direct => {
            if incoming_result.preamble.method == "CONNECT" {
                preamble_for_upstream = None;
                my_response_for_downstream = Some(b"HTTP/1.1 200 OK\r\n\r\n");
            } else {
                let uri = match uri::Uri::new(&incoming_result.preamble.uri) {
                    Ok(uri) => uri,
                    Err(_) => {
                        send_error(&mut downstream_connection).await;
                        return Ok(());
                    }
                };
                let mut p = incoming_result.preamble;
                p.uri = format!(
                    "{}{}",
                    uri.path.unwrap_or(String::from("/")),
                    uri.query.unwrap_or(String::from(""))
                );
                preamble_for_upstream = Some(p);
                my_response_for_downstream = None;
            }
            buffered_for_upstream = incoming_result.buffered;
            upstream_address = (host.clone(), port);
        }
        ProxySuggestion::Proxy { host, port } => {
            preamble_for_upstream = Some(incoming_result.preamble);
            buffered_for_upstream = incoming_result.buffered;
            my_response_for_downstream = None;
            upstream_address = (host, port.unwrap_or(3128))
        }
    }

    let upstream_socket_addr = match upstream_address {
        (host, port) => match (host.as_str(), port).to_socket_addrs() {
            Ok(mut iter) => iter
                .next()
                .expect("Parsed address successfully, but no result??"),
            Err(_) => {
                send_error(&mut downstream_connection).await;
                return Ok(());
            }
        },
    };
    debug!("Upstream resolved to: {:?}", upstream_socket_addr);

    let mut upstream_connection = net::TcpStream::connect(&upstream_socket_addr).await?;
    debug!("Connected to upstream");

    if let Some(preamble) = preamble_for_upstream {
        preamble.write(&mut upstream_connection).await?;
        upstream_connection
            .write_all(&buffered_for_upstream)
            .await?;
        trace!("Written preamble to upstream");
    }
    if let Some(response) = my_response_for_downstream {
        downstream_connection.write_all(&response).await?;
        trace!("Written my response to downstream");
    }
    trace!("Starting two-way pipe");
    two_way_pipe(&mut upstream_connection, &mut downstream_connection).await?;
    debug!("Successfully served request");
    return Ok(());
}
