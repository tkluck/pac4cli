use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

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

//fn choose_handler(request_line: connection::RequestLine) -> ConnectionHandler {
//
//}

pub fn run_server(port: u16) {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);
    let listener = TcpListener::bind(&addr).unwrap();

    let server = listener.incoming().for_each(|downstream_connection| {
        println!("accepted socket; addr={:?}", downstream_connection.peer_addr().unwrap());

        let task = connection::Incoming::new(downstream_connection)
            .and_then(|(incoming_result,downstream_connection)| {
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
                        let port : u16 = if uri.scheme == "https" { 443 } else { 80 };
                        (incoming_result.preamble.uri.clone(), host, port)
                    };

                let upstream_addr : SocketAddr;
                let preamble_for_upstream : Option<Preamble>;
                let my_response_for_downstream : Option<Vec<u8>>;

                match find_proxy_suggestions(url, host).first() {
                    Some(&ProxySuggestion::Direct) => {
                        if incoming_result.preamble.method == "CONNECT" {
                            preamble_for_upstream = None;
                            my_response_for_downstream = Some(b"HTTP/1.1 200 OK\r\n\r\n".to_vec());
                        } else {
                            let uri =  Uri::new(&incoming_result.preamble.uri).expect("Can't parse incoming uri");
                            let mut p = incoming_result.preamble.clone();
                            p.uri = format!("{}{}", uri.path.unwrap_or(String::from("/")), uri.query.unwrap_or(String::from("")));
                            preamble_for_upstream = Some(p);
                            my_response_for_downstream = None;
                        }
                        upstream_addr = (host.as_str(), port).to_socket_addrs().expect("unparseable host").next().unwrap();
                    }
                    Some(&ProxySuggestion::Proxy(..)) => panic!("Not implemented yet"),
                    None => panic!("No proxy suggestions?"),
                }
                println!("Host: {}, upstream addr: {:?}", host, upstream_addr);

                TcpStream::connect(&upstream_addr)
                    .and_then(move |upstream_connection| {
                        let write_upstream =
                            if let Some(preamble) = preamble_for_upstream {
                                Either::A(preamble.write(upstream_connection)
                                        .map(|(_preamble, upstream_connection)| {
                                            upstream_connection
                                        }))
                            } else {
                                Either::B(future::ok(upstream_connection))
                            };
                        let write_downstream =
                            if let Some(response) = my_response_for_downstream {
                                Either::A(io::write_all(downstream_connection, response)
                                        .map(|(downstream_connection, _response)| {
                                            downstream_connection
                                        }))
                            } else {
                                Either::B(future::ok(downstream_connection))
                            };
                        write_upstream.join(write_downstream)
                    })
                    .and_then(|(upstream_connection, downstream_connection)| {
                        two_way_pipe(upstream_connection, downstream_connection)
                    })
                    .and_then(|_| {
                        Ok(())
                    })
            })
            .map_err(|err| {
                println!("connection error = {:?}", err);
            });

        // Spawn a new task that processes the socket:
        tokio::spawn(task);

        Ok(())
    })
    .map_err(|err| {
        // All tasks must have an `Error` type of `()`. This forces error
        // handling and helps avoid silencing failures.
        //
        // In our example, we are only going to log the error to STDOUT.
        println!("accept error = {:?}", err);
    });
    println!("server running on {}", addr);
    tokio::run(server);
}
