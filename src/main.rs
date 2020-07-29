use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_scope;

use argparse::{ArgumentParser, StoreTrue, Store, StoreOption};
use ini::Ini;
use slog::Drain;
use slog_journald::JournaldDrain;
use tokio::signal::unix::{signal, SignalKind};
use tokio::net::TcpListener;

mod pacparser;
mod proxy;
mod wpad;
mod systemd;

use pacparser::{ProxySuggestion,find_proxy_suggestions};
use wpad::AutoConfigHandler;

#[derive(Clone)]
struct Options {
    config: Option<String>,
    port: u16,
    force_proxy: Option<ProxySuggestion>,
    force_wpad_url: Option<String>,
    loglevel: slog::FilterLevel,
    systemd: bool,
}

#[tokio::main]
async fn main() {

    let options = {
        let mut options = Options {
            config:      None,
            port:        3128,
            force_proxy: None,
            force_wpad_url: None,
            loglevel:    slog::FilterLevel::Debug,
            systemd:     false,
        };
        {
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

        options.force_wpad_url = if let Some(file) = options.config.clone() {
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
        options.clone()
    };
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
    let find_proxy_arc = Arc::new(find_proxy);

    update_configuration_from_wpad(&options, auto_config_handler.clone()).await;

    let mut sighups = signal(SignalKind::hangup()).unwrap();
    let mut sigints = signal(SignalKind::interrupt()).unwrap();

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), options.port);
    let mut listener = TcpListener::bind(&addr).await.unwrap();

    systemd::notify_ready();

    loop {
        tokio::select! {
            _ = sighups.recv() => {
                update_configuration_from_wpad(&options, auto_config_handler.clone()).await;
            }
            _ = sigints.recv() => {
                break;
            }
            Ok((downstream_connection, _)) = listener.accept() => {
                debug!("accepted socket; addr={:?}", downstream_connection.peer_addr().unwrap());
                let find_proxy_clone = find_proxy_arc.clone();
                tokio::spawn(async move {
                    proxy::process_socket(downstream_connection, find_proxy_clone).await;
                });
            }
        }
    }

    pacparser::cleanup();

    info!("Clean exit");
}

async fn update_configuration_from_wpad(options: &Options, auto_config_handler: Arc<AutoConfigHandler>) {
    if options.force_proxy.is_none() {
        let urls = match options.force_wpad_url {
            Some(ref url) => [url.clone()].to_vec(),
            None => wpad::get_wpad_urls().await.unwrap(),
        };
        let wpad = wpad::retrieve_first_working_url(urls).await.unwrap();
        auto_config_handler.set_current_wpad_script(wpad);
    }
}
