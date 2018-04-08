extern crate tokio;

use std::io::prelude::*;
use std::fs::File;

use tokio::io;
use tokio::net::TcpListener;
use tokio::prelude::*;

mod pacparser;

fn main() {
    pacparser::init().expect("Failed to initialize pacparser");

    let mut wpadfile = File::open("test/wpadserver/wpad.dat").expect("File not found");
    let mut wpadtext = String::new();
    wpadfile.read_to_string(&mut wpadtext).expect("Couldn't read from file");

    pacparser::parse_pac_string(wpadtext).expect("Couldn't parse wpad file");

    let proxystr = pacparser::find_proxy("http://www.google.com", "www.google.com");
    println!("proxystr: {}", proxystr);

    let addr = "127.0.0.1:6142".parse().unwrap();
    let listener = TcpListener::bind(&addr).unwrap();

    let server = listener.incoming().for_each(|socket| {
        println!("accepted socket; addr={:?}", socket.peer_addr().unwrap());

        let connection = io::write_all(socket, "hello world\n")
            .then(|res| {
                println!("wrote message; success={:?}", res.is_ok());
                Ok(())
        });

        // Spawn a new task that processes the socket:
        tokio::spawn(connection);

        Ok(())
    })
    .map_err(|err| {
        // All tasks must have an `Error` type of `()`. This forces error
        // handling and helps avoid silencing failures.
        //
        // In our example, we are only going to log the error to STDOUT.
        println!("accept error = {:?}", err);
    });
    println!("server running on localhost:6142");
    tokio::run(server);
}
