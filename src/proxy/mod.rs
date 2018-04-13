use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::{Mutex,Arc};

use tokio;
use tokio::io;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::prelude::*;
use futures::future::Either;

use uri::Uri;

mod connection;
mod protocol;

use self::connection::two_way_pipe;
use self::protocol::Preamble;
use pacparser::{ProxySuggestion,find_proxy_suggestions};
use wpad;

fn find_proxy(url: &str, host: &str, forced_proxy: Option<ProxySuggestion>, auto_config_state: Arc<Mutex<wpad::AutoConfigState>>) -> ProxySuggestion {
    let state = auto_config_state.lock().expect("Issue locking auto config state");
    match *state {
        wpad::AutoConfigState::Discovering => ProxySuggestion::Direct,
        wpad::AutoConfigState::Direct => ProxySuggestion::Direct,
        wpad::AutoConfigState::PAC => match forced_proxy {
            Some(ref proxy_suggestion) => proxy_suggestion.clone(),
            None => find_proxy_suggestions(url, host).remove(0),
        },
    }
}

pub fn create_server(port: u16, forced_proxy: Option<ProxySuggestion>, auto_config_state: Arc<Mutex<wpad::AutoConfigState>>) -> Box<Future<Item=(),Error=()>+Send> {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
    let listener = TcpListener::bind(&addr).unwrap();

    let server = listener.incoming().for_each(move |downstream_connection| {
        debug!("accepted socket; addr={:?}", downstream_connection.peer_addr().unwrap());

        // make a copy for the nested closure
        let forced_proxy = forced_proxy.clone();
        let auto_config_state = auto_config_state.clone();

        let task = connection::Incoming::new(downstream_connection)
        .and_then(move |(incoming_result,downstream_connection)| {
            let error_future = |downstream_connection| {
                Either::A(io::write_all(downstream_connection, b"<h1>Could not connect</h1>").map(|_| ()))
            };
            // make a copy for the nested closure
            let forced_proxy = forced_proxy.clone();
            let auto_config_state = auto_config_state.clone();
            // First, find a hostname + url for doing the pacparser lookup
            let (ref url, ref host, port) =
                if incoming_result.preamble.method == "CONNECT" {
                    let mut parts = incoming_result.preamble.uri.split(":");
                    let host = String::from(parts.next().expect("No host in connect spec"));
                    let port = parts.next().expect("No port in connect spec").parse::<u16>().expect("Invalid port in connect spec");
                    (host.clone(), host, port)
                } else {
                    let uri =  Uri::new(&incoming_result.preamble.uri).expect("Can't parse incoming uri");
                    let host = uri.host.expect("No host in URI; aborting");
                    let default_port : u16 = if uri.scheme == "https" { 443 } else { 80 };
                    let port = uri.port.unwrap_or(default_port);
                    (incoming_result.preamble.uri.clone(), host, port)
                };
            debug!("Destination is {}:{}", host, port);

            let upstream_address : (String, u16);
            let preamble_for_upstream : Option<Preamble>;
            let my_response_for_downstream : Option<&'static [u8]>;

            match find_proxy(url, host, forced_proxy, auto_config_state) {
                ProxySuggestion::Direct => {
                    if incoming_result.preamble.method == "CONNECT" {
                        preamble_for_upstream = None;
                        my_response_for_downstream = Some(b"HTTP/1.1 200 OK\r\n\r\n");
                    } else {
                        let uri =  match Uri::new(&incoming_result.preamble.uri) {
                            Ok(uri) => uri,
                            Err(_) => return error_future(downstream_connection),
                        };
                        let mut p = incoming_result.preamble.clone();
                        p.uri = format!("{}{}", uri.path.unwrap_or(String::from("/")), uri.query.unwrap_or(String::from("")));
                        preamble_for_upstream = Some(p);
                        my_response_for_downstream = None;
                    }
                    upstream_address = (host.clone(), port);
                }
                ProxySuggestion::Proxy{host, port} => {
                    preamble_for_upstream = Some(incoming_result.preamble);
                    my_response_for_downstream = None;
                    upstream_address = (host, port.unwrap_or(3128))
                }
            }

            let upstream_socket_addr = match upstream_address {
                (host, port) => match (host.as_str(), port).to_socket_addrs() {
                    Ok(mut iter) => iter.next().expect("Parsed address successfully, but no result??"),
                    Err(_) => return error_future(downstream_connection),
                }
            };
            debug!("Upstream resolved to: {:?}", upstream_socket_addr);

            let data_exchange_future = TcpStream::connect(&upstream_socket_addr)
            .and_then(move |upstream_connection| {
                debug!("Connected to upstream");
                let write_upstream =
                    if let Some(preamble) = preamble_for_upstream {
                        Either::A(preamble.write(upstream_connection)
                        .map(|(_preamble, upstream_connection)| {
                            trace!("Written preamble to upstream");
                            upstream_connection
                        }))
                    } else {
                        Either::B(future::ok(upstream_connection))
                    };
                let write_downstream =
                    if let Some(response) = my_response_for_downstream {
                        Either::A(io::write_all(downstream_connection, response)
                        .map(|(downstream_connection, _response)| {
                            trace!("Written my response to downstream");
                            downstream_connection
                        }))
                    } else {
                        Either::B(future::ok(downstream_connection))
                    };
                write_upstream.join(write_downstream)
            })
            .and_then(|(upstream_connection, downstream_connection)| {
                trace!("Starting two-way pipe");
                two_way_pipe(upstream_connection, downstream_connection)
            })
            .map(|_| {
                debug!("Successfully served request");
            });
            Either::B(data_exchange_future)
        })
        .map_err(|err| {
            debug!("error parsing incoming request: {:?}", err);
        });

        // Spawn a new task that processes the socket:
        tokio::spawn(task);

        Ok(())
    })
    .map_err(|err| {
        error!("accept error: {:?}", err);
    });
    return Box::new(server);
}
