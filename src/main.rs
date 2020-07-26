
use std::sync::Arc;

#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_scope;

use argparse::{ArgumentParser, StoreTrue, Store, StoreOption};
use ini::Ini;
use slog::Drain;
use slog_journald::JournaldDrain;
use tokio_signal::unix::{Signal, SIGHUP};

mod pacparser;
mod proxy;
mod wpad;
mod systemd;

use pacparser::{ProxySuggestion,find_proxy_suggestions};
use wpad::AutoConfigHandler;

struct Options {
    config: Option<String>,
    port: u16,
    force_proxy: Option<ProxySuggestion>,
    loglevel: slog::FilterLevel,
    systemd: bool,
}

#[tokio::main]
async fn main() {
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

    let auto_config_handler = Arc::new(AutoConfigHandler::new());

    let find_proxy = {
        let auto_config_state = auto_config_handler.get_state_ref();
        let forced_proxy = options.force_proxy.clone();
        move |url: &str, host: &str| {
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
    };


    /*
    let handle_sighups : Box<dyn Future<Item=(),Error=()>> = {
        if options.force_proxy.is_none() && force_wpad_url.is_none() {
            let handle = core.handle();
            let auto_config_handler = auto_config_handler.clone();
            let task = Signal::new(SIGHUP, &handle).flatten_stream()
            .map_err(|err| {
                warn!("Error retrieving SIGHUPs: {:?}", err)
            })
            .and_then(move |_| {
                let handle = handle.clone();
                let auto_config_handler = auto_config_handler.clone();
                info!("SIGHUP received");
                wpad::get_wpad_urls(&handle)
                .and_then(move |urls| {
                    wpad::retrieve_first_working_url(&handle, urls)
                })
                .map(move |wpad| {
                    auto_config_handler.set_current_wpad_script(wpad)
                })
            })
            .for_each(|_| {
                future::ok(())
            })
            .map_err(|err| {
                warn!("Error handling SIGHUP: {:?}", err)
            });
            Box::new(task)
        } else {
            // if we get the proxy from arguments or the config file,
            // no need to listen for SIGHUPs.
            Box::new(future::ok(()))
        }
    };
    */

    if options.force_proxy.is_none() {
        let urls = match force_wpad_url {
            Some(ref url) => [url.clone()].to_vec(),
            None => wpad::get_wpad_urls().await.unwrap(),
        };
        let wpad = wpad::retrieve_first_working_url(urls).await.unwrap();
        auto_config_handler.set_current_wpad_script(wpad)
    }

    // there is still a race condition here, as the socket is
    // only bound lazily by tokio's futures/streams. The API has
    // no way of hooking in to the moment that the socket has been
    // bound (before any connections have been accepted), so this
    // is as close as we'll get.
    systemd::notify_ready();

    proxy::serve(options.port, find_proxy).await;

    pacparser::cleanup();

    info!("Clean exit");
}
