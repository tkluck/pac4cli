use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

#[macro_use]
extern crate slog;
#[macro_use]
extern crate slog_scope;

use slog::Drain;
use slog_journald::JournaldDrain;
use tokio::signal::unix::{signal, SignalKind};
use tokio::net::TcpListener;
use structopt::StructOpt;

mod networkmanager;
mod pacparser;
mod options;
mod proxy;
mod wpad;
mod systemd;

#[tokio::main]
async fn main() {

    let flags = options::CmdLineOptions::from_args();
    let options = options::Options::load(&flags);

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

    let network_env = networkmanager::NetworkManager::new();

    let proxy_resolver = wpad::ProxyResolver::load(network_env, flags.clone()).await;
    let proxy_resolver_ref = Arc::new(proxy_resolver);

    let mut sighups = signal(SignalKind::hangup()).unwrap();
    let mut sigints = signal(SignalKind::interrupt()).unwrap();

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), options.port);
    { // scope for listener
        let mut listener = TcpListener::bind(&addr).await.unwrap();

        systemd::notify_ready();

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
