use std::net::{IpAddr, Ipv4Addr, SocketAddr};

extern crate tokio;
use self::tokio::io;
use self::tokio::net::TcpListener;
use self::tokio::prelude::*;

mod connection;

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
            .and_then(|incoming_result| {
                //connection_handler = choose_handler(request_line);
                //connection_handler.handle(request_line, headers, io);
                println!("{:?}", incoming_result);
                Ok(())
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
