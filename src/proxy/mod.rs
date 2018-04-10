use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

use tokio;
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

    let server = listener.incoming().for_each(|socket| {
        println!("accepted socket; addr={:?}", socket.peer_addr().unwrap());

        let task = connection::Incoming::new(socket)
            .and_then(|(incoming_result,socket)| {
                let uri =  Uri::new(&incoming_result.preamble.uri).expect("Can't parse incoming uri");
                let host = uri.host.expect("No host in URI; aborting");

                let preamble_for_upstream : Option<Preamble>;
                let remote_addr : SocketAddr;

                match find_proxy_suggestions(&incoming_result.preamble.uri, &host).first() {
                    Some(&ProxySuggestion::Direct) => {
                        if incoming_result.preamble.method == "CONNECT" {
                            preamble_for_upstream = None;
                        } else {
                            let mut p = incoming_result.preamble.clone();
                            p.uri = format!("{}{}", uri.path.unwrap_or(String::from("/")), uri.query.unwrap_or(String::from("")));
                            preamble_for_upstream = Some(p);
                        }
                        remote_addr = (host.as_str(), uri.port.unwrap_or(80)).to_socket_addrs().expect("unparseable host").next().unwrap();
                    }
                    Some(&ProxySuggestion::Proxy(..)) => panic!("Not implemented yet"),
                    None => panic!("No proxy suggestions?"),
                }

                let data_exchange = TcpStream::connect(&remote_addr)
                    .and_then(move |upstream_connection| {
                        if let Some(preamble) = preamble_for_upstream {
                            Either::A(preamble.write(upstream_connection)
                                    .map(|(_preamble, upstream_connection)| {
                                        upstream_connection
                                    }))
                        } else {
                            Either::B(future::ok(upstream_connection))
                        }
                    })
                    .and_then(move |upstream_connection| {
                        two_way_pipe(upstream_connection, socket)
                    })
                    .and_then(|_| {
                        Ok(())
                    });

                data_exchange
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
