extern crate argparse;
extern crate tokio;
extern crate tokio_core;
#[macro_use]
extern crate futures;
extern crate uri;
extern crate dbus;
extern crate dbus_tokio;
extern crate hyper;

// Needed since https://github.com/tokio-rs/tokio/commit/a6b307cfbefb568bd79eaf1d91edf9ab52d18533#diff-b4aea3e418ccdb71239b96952d9cddb6
// is not released yet.
extern crate tokio_io;

use std::sync::{Mutex,Arc};

use argparse::{ArgumentParser, StoreTrue, Store, StoreOption};
use tokio_core::reactor::Core;
use futures::Future;

mod pacparser;
mod proxy;
mod wpad;

use pacparser::ProxySuggestion;

struct Options {
    config: Option<String>,
    port: u16,
    force_proxy: Option<ProxySuggestion>,
    loglevel: String,
    systemd: bool,
}

#[derive(Debug,Clone)]
pub enum AutoConfigState {
    Discovering,
    Direct,
    PAC,
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

    let auto_config_state = Arc::new(Mutex::new(AutoConfigState::Discovering));

    let mut core = Core::new().unwrap();

    let set_wpad_config = {
        let auto_config_state = auto_config_state.clone();
        wpad::get_wpad_file(&mut core)
        .map(move |wpad| {
            let mut state = auto_config_state.lock().expect("issue locking state");
            *state = if let Some(ref script) = wpad {
                match pacparser::parse_pac_string(script) {
                    Ok(..) => AutoConfigState::PAC,
                    Err(..) => AutoConfigState::Direct,
                }
            } else {
                AutoConfigState::Direct
            };
            println!("State is now {:?}", *state)
        })
    };
    let serve = {
        let auto_config_state = auto_config_state.clone();
        proxy::create_server(options.port, options.force_proxy, auto_config_state)
    };

    let start_server = set_wpad_config
    .and_then(|_| {
        serve
    });

    core.run(start_server);

    pacparser::cleanup();
}
