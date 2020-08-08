use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync;

#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_scope;

use slog::Drain;
#[cfg(feature="slog-journald")]
use slog_journald::JournaldDrain;
use structopt::StructOpt;
#[cfg(feature="systemd")]
use systemd::daemon;
use tokio::net;
use tokio::signal::unix;

mod connection;
#[cfg_attr(feature="network-manager", path="networkmanager.rs")]
#[cfg_attr(not(feature="network-manager"), path="networkmanager-stub.rs")]
mod networkmanager;
mod options;
mod pacparser;
mod proxy;
mod wpad;

#[tokio::main]
async fn main() {
    let flags = options::CmdLineOptions::from_args();
    let options = options::Options::load(&flags);

    // set up logging
    // Need to keep _guard alive for as long as we want to log
    let _guard = slog_scope::set_global_logger(get_logger(options.systemd));

    pacparser::init().expect("Failed to initialize pacparser");

    let network_env = networkmanager::NetworkManager::new();

    let proxy_resolver = wpad::ProxyResolver::load(network_env, flags.clone()).await;
    let proxy_resolver_ref = sync::Arc::new(proxy_resolver);

    let mut sighups = unix::signal(unix::SignalKind::hangup()).unwrap();
    let mut sigints = unix::signal(unix::SignalKind::interrupt()).unwrap();

    // scope for listener
    {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), options.port);
        let mut listener = net::TcpListener::bind(&addr).await.unwrap();

        notify();

        loop {
            tokio::select! {
                Ok((downstream_connection, _)) = listener.accept() => {
                    debug!("accepted socket; addr={:?}", downstream_connection.peer_addr().unwrap());
                    let proxy_resolver_ref = proxy_resolver_ref.clone();
                    tokio::spawn(async move {
                        let proxy_resolver = &*proxy_resolver_ref;
                        let res = proxy::process_socket(downstream_connection, proxy_resolver).await;
                        if let Err(err) = res {
                            warn!("Issue while handling connection: {:?}", err);
                        }
                    });
                }
                _ = sigints.recv() => {
                    break;
                }
                _ = sighups.recv() => {
                    proxy_resolver_ref.reload().await;
                }
            }
        }
    }

    // TODO: do we need to wait for spawned tasks explicitly?

    pacparser::cleanup();

    info!("Clean exit");
}

#[cfg(feature="systemd")]
fn notify() {
    info!("Notifying with systemd");
    daemon::notify(false, [(daemon::STATE_READY, "1")].iter());
}

#[cfg(not(feature="systemd"))]
fn notify() {
    info!("Notifying without systemd");
}


#[cfg(feature="slog-journald")]
fn get_logger(use_journald: bool) -> slog::Logger {
    if use_journald {
        let drain = JournaldDrain.ignore_res();
        slog::Logger::root(drain, slog_o!())
    } else {
        get_terminal_logger()
    }
}

#[cfg(not(feature="slog-journald"))]
fn get_logger(_use_journald: bool) -> slog::Logger {
    get_terminal_logger()
}

fn get_terminal_logger() -> slog::Logger {
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    let drain = slog_term::FullFormat::new(plain).build().fuse();
    slog::Logger::root(drain, slog_o!())
}
