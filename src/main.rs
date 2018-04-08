extern crate tokio;
extern crate argparse;

use std::io::prelude::*;
use std::fs::File;

use argparse::{ArgumentParser, StoreTrue, Store};
use tokio::io;
use tokio::net::TcpListener;
use tokio::prelude::*;

mod pacparser;

struct Options {
    config: String,
    port: i32,
    force_proxy: String,
    loglevel: String,
    systemd: bool,
}

fn main() {
    let mut options = Options {
        config:      String::new(),
        port:        3128,
        force_proxy: String::new(),
        loglevel:    String::from("DEBUG"),
        systemd:     false,
    };

    {  // this block limits scope of borrows by ap.refer() method
        let mut ap = ArgumentParser::new();
        ap.set_description("
        Run a simple HTTP proxy on localhost that uses a wpad.dat to decide
        how to connect to the actual server.
        ");
        ap.refer(&mut options.config)
            .add_option(&["-c", "--config"], Store,
            "Path to configuration file [not implemented]");
        ap.refer(&mut options.port)
            .metavar("PORT")
            .add_option(&["-p","--port"], Store,
            "Port to listen on");
        ap.refer(&mut options.force_proxy)
            .metavar("PROXY STRING")
            .add_option(&["-F", "--force-proxy"], Store,
            "Forward traffic according to PROXY STRING, e.g. DIRECT or PROXY <proxy>");
        ap.refer(&mut options.loglevel)
            .metavar("LEVEL")
            .add_option(&["--loglevel"], Store,
            "One of DEBUG/INFO/WARNING/ERROR");
        ap.refer(&mut options.systemd)
            .add_option(&["--systemd"], StoreTrue,
            "Assume running under systemd (for logging and readiness notification) [not implemented]");
        ap.parse_args_or_exit();
    }
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

    pacparser::cleanup();
}
