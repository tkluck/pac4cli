#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_scope;
extern crate slog_term;
extern crate slog_journald;

extern crate argparse;
extern crate ini;
extern crate tokio;
extern crate tokio_core;
#[macro_use]
extern crate futures;
extern crate uri;
extern crate dbus;
extern crate dbus_tokio;
extern crate tokio_signal;
extern crate hyper;

// Needed since https://github.com/tokio-rs/tokio/commit/a6b307cfbefb568bd79eaf1d91edf9ab52d18533#diff-b4aea3e418ccdb71239b96952d9cddb6
// is not released yet.
extern crate tokio_io;

use std::sync::{Mutex,Arc};

use argparse::{ArgumentParser, StoreTrue, Store, StoreOption};
use ini::Ini;
use slog::Drain;
use slog_journald::JournaldDrain;
use tokio_core::reactor::{Core,Handle};
use futures::{Future,Stream};
use futures::future;
use futures::future::Either;
use tokio_signal::unix::{Signal, SIGHUP};

mod pacparser;
mod proxy;
mod wpad;
mod systemd;

use pacparser::ProxySuggestion;

struct Options {
    config: Option<String>,
    port: u16,
    force_proxy: Option<ProxySuggestion>,
    loglevel: slog::FilterLevel,
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
        loglevel:    slog::FilterLevel::Debug,
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
            "Path to configuration file");
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
            "Assume running under systemd (log to journald)");
        ap.parse_args_or_exit();
    }
    // set up logging
    // Need to keep _guard alive for as long as we want to log
    let _guard = match options.systemd {
        false => {
            let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
            let drain = slog_term::FullFormat::new(plain).build().fuse();
            let log = slog::Logger::root(drain, slog_o!());
            slog_scope::set_global_logger(log)
        }
        true => {
            let drain = JournaldDrain.ignore_res();
            let log = slog::Logger::root(drain, slog_o!());
            slog_scope::set_global_logger(log)
        }
    };
    slog_scope::scope(&slog_scope::logger().new(slog_o!()), || {

        let force_wpad_url = if let Some(file) = options.config {
            info!("Loading configuration file {}", file);
            let conf = Ini::load_from_file(file).expect("Failed to load config file");
            if let Some(section) = conf.section(Some("wpad".to_owned())) {
                if let Some(url) = section.get("url") {
                    Some(url.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        pacparser::init().expect("Failed to initialize pacparser");

        let auto_config_state = Arc::new(Mutex::new(AutoConfigState::Discovering));

        let mut core = Core::new().unwrap();

        fn find_wpad_config_future(force_wpad_url: &Option<String>, auto_config_state: &Arc<Mutex<AutoConfigState>>, handle: &Handle) -> Box<Future<Item=(), Error=()>> {
            let auto_config_state = auto_config_state.clone();
            let get_urls = if let &Some(ref url) = force_wpad_url {
                Either::A(future::ok([url.clone()].to_vec()))
            } else {
                let handle = handle.clone();
                Either::B(wpad::get_wpad_urls(handle))
            };
            let handle = handle.clone();
            let task = get_urls
            .and_then(|urls| {
                wpad::retrieve_first_working_url(handle, urls)
            })
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
                info!("State is now {:?}", *state)
            });
            Box::new(task)
        }

        let serve = {
            let auto_config_state = auto_config_state.clone();
            proxy::create_server(options.port, options.force_proxy, auto_config_state)
        };

        let handle_sighups = {
            let handle = core.handle();
            let auto_config_state = auto_config_state.clone();
            let force_wpad_url = force_wpad_url.clone();
            Signal::new(SIGHUP, &handle).flatten_stream()
            .map_err(|err| {
                warn!("Error retrieving SIGHUPs: {:?}", err)
            })
            .for_each(move |_| {
                info!("SIGHUP received");
                find_wpad_config_future(&force_wpad_url, &auto_config_state, &handle)
            })
            .map_err(|err| {
                warn!("Error handling SIGHUP: {:?}", err)
            })
        };

        let start_server = find_wpad_config_future(&force_wpad_url, &auto_config_state, &core.handle())
        .and_then(|_| {
            serve.join(handle_sighups)
        })
        .map_err(|err| {
            error!("Can't start server: {:?}", err)
        });

        // there is still a race condition here, as the socket is
        // only bound lazily by tokio's futures/streams. The API has
        // no way of hooking in to the moment that the socket has been
        // bound (before any connections have been accepted), so this
        // is as close as we'll get.
        systemd::notify_ready();

        core.run(start_server).expect("Issue running server!");

        pacparser::cleanup();
    });
}
