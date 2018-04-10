use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};

use tokio;
use tokio::io;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::prelude::*;

use uri::Uri;

mod connection;
mod protocol;

use self::connection::two_way_pipe;

struct Pac4CliProxy;

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
                //connection_handler = choose_handler(request_line);
                //connection_handler.handle(request_line, headers, io);
                let uri =  Uri::new(&incoming_result.preamble.uri).expect("Can't parse incoming uri");

                let mut remote_addr = (uri.host.unwrap().as_str(), uri.port.unwrap_or(80)).to_socket_addrs().expect("unparseable host");

                let data_exchange = TcpStream::connect(&remote_addr.next().unwrap())
                    .and_then(move |upstream_connection| {
                        incoming_result.preamble.write(upstream_connection)
                    })
                    .and_then(move |(preamble, upstream_connection)| {
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
