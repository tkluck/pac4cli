extern crate argparse;
extern crate tokio;
#[macro_use]
extern crate futures;
extern crate uri;

// Needed since https://github.com/tokio-rs/tokio/commit/a6b307cfbefb568bd79eaf1d91edf9ab52d18533#diff-b4aea3e418ccdb71239b96952d9cddb6
// is not released yet.
extern crate tokio_io;

use std::io::prelude::*;
use std::fs::File;

use argparse::{ArgumentParser, StoreTrue, Store, StoreOption};

mod pacparser;
mod proxy;

use pacparser::ProxySuggestion;

struct Options {
    config: Option<String>,
    port: u16,
    force_proxy: Option<ProxySuggestion>,
    loglevel: String,
    systemd: bool,
}

fn main() {
    let mut options = Options {
        config:      None,
        port:        3128,
        force_proxy: None,
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
            .add_option(&["-c", "--config"], StoreOption,
            "Path to configuration file [not implemented]");
        ap.refer(&mut options.port)
            .metavar("PORT")
            .add_option(&["-p","--port"], Store,
            "Port to listen on");
        ap.refer(&mut options.force_proxy)
            .metavar("PROXY STRING")
            .add_option(&["-F", "--force-proxy"], StoreOption,
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

    proxy::run_server(options.port, options.force_proxy);

    pacparser::cleanup();
}
