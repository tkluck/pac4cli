use std::rc::Rc;
use std::collections::HashMap;
use std::sync::{Mutex,Arc};

use dbus;
use dbus::{BusType,Connection,Message,Path};
use dbus::arg::Variant;
use tokio::prelude::*;
use tokio_core::reactor::Handle;
use futures::future::{loop_fn,Either,Loop};
use dbus_tokio::AConnection;
use hyper;
use hyper::{Client,StatusCode};

use pacparser;

fn get_list_of_paths<P>(aconn: &AConnection, object_path: P, interface: &str, property: &str) -> Box<dyn Future<Item=Vec<Path<'static>>, Error=dbus::Error>>
    where P: Into<Path<'static>> {
    let m = Message::new_method_call("org.freedesktop.NetworkManager", object_path, "org.freedesktop.DBus.Properties", "Get").unwrap().append2(interface, property);
    let method_call = aconn.method_call(m).unwrap()
        .map(|m| {
            let res : Variant<Vec<Path<'static>>> = m.get1().expect("failed to parse list of paths");
            res.0
        });
    return Box::new(method_call);
}

fn get_path<P>(aconn: &AConnection, object_path: P, interface: &str, property: &str) -> Box<dyn Future<Item=Path<'static>, Error=dbus::Error>>
    where P: Into<Path<'static>> {
    let m = Message::new_method_call("org.freedesktop.NetworkManager", object_path, "org.freedesktop.DBus.Properties", "Get").unwrap().append2(interface, property);
    let method_call = aconn.method_call(m).unwrap()
        .map(|m| {
            let res : Variant<Path<'static>> = m.get1().expect("failed to parse path");
            res.0
        });
    return Box::new(method_call);
}

fn get_dict<P>(aconn: &AConnection, object_path: P, interface: &str, property: &str) -> Box<dyn Future<Item=HashMap<String,Variant<String>>, Error=dbus::Error>>
    where P: Into<Path<'static>> {
    let m = Message::new_method_call("org.freedesktop.NetworkManager", object_path, "org.freedesktop.DBus.Properties", "Get").unwrap().append2(interface, property);
    let method_call = aconn.method_call(m).unwrap()
        .map(|m| {
            let res : Variant<HashMap<String,Variant<String>>> = m.get1().expect("failed to parse dict");
            res.0
        });
    return Box::new(method_call);
}

fn get_list_of_strings<P>(aconn: &AConnection, object_path: P, interface: &str, property: &str) -> Box<dyn Future<Item=Vec<String>, Error=dbus::Error>>
    where P: Into<Path<'static>> {
    let m = Message::new_method_call("org.freedesktop.NetworkManager", object_path, "org.freedesktop.DBus.Properties", "Get").unwrap().append2(interface, property);
    let method_call = aconn.method_call(m).unwrap()
        .map(|m| {
            let res : Variant<Vec<String>> = m.get1().expect("failed to parse list of strings");
            res.0
        });
    return Box::new(method_call);
}

enum State {
    Start,
    ReceiveActiveConnections {
        paths_future: Box<dyn Future<Item=Vec<Path<'static>>,Error=dbus::Error>>,
    },
    LoopConnections,
    ReceiveDhcp4Config {
        dhcp4_config_future: Box<dyn Future<Item=Path<'static>,Error=dbus::Error>>,
    },
    ReceiveDhcp4Options {
        dhcp4_options_future: Box<dyn Future<Item=HashMap<String,Variant<String>>,Error=dbus::Error>>
    },
    ReceiveIP4Config {
        ip4_config_future: Box<dyn Future<Item=Path<'static>,Error=dbus::Error>>,
    },
    ReceiveDomain {
        domain_future: Box<dyn Future<Item=Vec<String>,Error=dbus::Error>>,
    },
    NextConnection,
    Done,
}

#[derive(Clone,Debug)]
struct WPADInfo {
    wpad_option: Option<String>,
    domains: Vec<String>,
}

struct WPADDiscoverer {
    aconn: AConnection,
    active_connections: Vec<Path<'static>>,
    ix: usize,
    state: State,
    wpad_info: WPADInfo,
}

impl WPADDiscoverer {
    fn new(handle: &Handle) -> Self {
        let c = Rc::new(Connection::get_private(BusType::System).unwrap());
        let aconn = AConnection::new(c.clone(), handle.clone()).unwrap();
        Self {
            aconn,
            active_connections: Vec::new(),
            ix: 0,
            state: State::Start,
            wpad_info: WPADInfo {
                wpad_option: None,
                domains: Vec::new(),
            }
        }
    }
}

impl Future for WPADDiscoverer {
    type Item = WPADInfo;
    type Error = dbus::Error;
     fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
         loop {
             self.state = match self.state {
                 State::Start => {
                     debug!("Finding active connections");
                     let paths_future = get_list_of_paths(&self.aconn, "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager", "ActiveConnections");
                     State::ReceiveActiveConnections { paths_future }
                 }
                 State::ReceiveActiveConnections { ref mut paths_future } => {
                     let active_connections = try_ready!(paths_future.poll());
                     debug!("received active connections: {:?}", active_connections);
                     self.active_connections.extend(active_connections);
                     State::LoopConnections
                 }
                 State::LoopConnections => {
                     if self.ix < self.active_connections.len() {
                         let dhcp4_config_future = get_path(&self.aconn, self.active_connections[self.ix].clone(), "org.freedesktop.NetworkManager.Connection.Active", "Dhcp4Config");
                         State::ReceiveDhcp4Config { dhcp4_config_future }
                     } else {
                         State::Done
                     }
                 }
                 State::ReceiveDhcp4Config { ref mut dhcp4_config_future } => {
                     let config_path = try_ready!(dhcp4_config_future.poll());
                     debug!("received config path: {:?}", config_path);
                     let dhcp4_options_future = get_dict(&self.aconn, config_path.clone(), "org.freedesktop.NetworkManager.DHCP4Config", "Options");
                     State::ReceiveDhcp4Options { dhcp4_options_future }
                 }
                 State::ReceiveDhcp4Options { ref mut dhcp4_options_future } => {
                     let options = try_ready!(dhcp4_options_future.poll());
                     debug!("received dhcp4 options: {:?}", options);
                     self.wpad_info.wpad_option = match options.get(&String::from("wpad")) {
                         None => None,
                         Some(s) => Some(s.0.clone()),
                     };
                     let ip4_config_future = get_path(&self.aconn, self.active_connections[self.ix].clone(), "org.freedesktop.NetworkManager.Connection.Active", "Ip4Config");
                     State::ReceiveIP4Config { ip4_config_future }
                 }
                 State::ReceiveIP4Config { ref mut ip4_config_future } => {
                     debug!("polling ip4 config");
                     let config_path = try_ready!(ip4_config_future.poll());
                     debug!("received config path: {:?}", config_path);
                     let domain_future = get_list_of_strings(&self.aconn, config_path.clone(), "org.freedesktop.NetworkManager.IP4Config", "Domains");
                     State::ReceiveDomain { domain_future }
                 }
                 State::ReceiveDomain { ref mut domain_future } => {
                     let domains = try_ready!(domain_future.poll());
                     debug!("received domains: {:?}", domains);
                     self.wpad_info.domains.extend(domains);
                     State::NextConnection
                 }
                 State::NextConnection => {
                     self.ix += 1;
                     State::LoopConnections
                 }
                 State::Done => return Ok(Async::Ready(self.wpad_info.clone())),
             };
         }
     }
 }

pub fn get_wpad_urls(handle: &Handle) -> Box<dyn Future<Item=Vec<String>,Error=()>> {

    let task = WPADDiscoverer::new(handle)
    .map_err(|dbus_err| {
        warn!("dbus error: {:?}", dbus_err)
    })
    .map(|info| {
        info!("Found network information: {:?}", info);
        let url_strings = match info.wpad_option {
            None => info.domains.iter().map(|d| {
                format!("http://wpad.{}/wpad.dat", d)
            }).collect(),
            Some(url) => [url].to_vec(),
        };
        url_strings
    });
    return Box::new(task);
}

pub fn retrieve_first_working_url(handle: &Handle, url_strings: Vec<String>) -> Box<dyn Future<Item=Option<String>,Error=()>> {

    let http_client = Client::new(handle);

    let urls :Vec<hyper::Uri> = url_strings.iter().filter_map(|url| url.parse::<hyper::Uri>().ok()).collect();
    debug!("Urls: {:?}", urls);

    let n = urls.len();
    let task = loop_fn(0, move |ix| {
        if ix < n {
            let get_wpad = http_client.get(urls[ix].clone())
            .and_then(move |res| {
                if res.status() != StatusCode::Ok {
                    Either::A(future::ok(Loop::Continue(ix+1)))
                } else {
                    Either::B(res.body().concat2().map(|body| {
                        let wpad_script = String::from_utf8(body.to_vec()).expect("wpad script not valid utf8");
                        trace!("wpad script: {}", wpad_script);
                        Loop::Break(Some(wpad_script))
                    }))
                }
            })
            .or_else(move |err| {
                // this error is expected, as we're just sending requests
                // to random wpad.<domain> hosts that don't even exist
                // in most networks
                info!("No wpad configuration found: {:?}", err);
                future::ok(Loop::Continue(ix+1))
            });
            Either::A(get_wpad)
        } else {
            Either::B(future::ok(Loop::Break(None)))
        }
    });
    return Box::new(task);
}

#[derive(Debug,Clone)]
pub enum AutoConfigState {
    Discovering,
    Direct,
    PAC,
}

pub struct AutoConfigHandler {
    state: Arc<Mutex<AutoConfigState>>,
}

impl AutoConfigHandler {
    pub fn new() -> Self {
        AutoConfigHandler {
            state: Arc::new(Mutex::new(AutoConfigState::Discovering)),
        }
    }

    pub fn get_state_ref(&self) -> Arc<Mutex<AutoConfigState>> {
        self.state.clone()
    }

    pub fn set_current_wpad_script(&self, wpad: Option<String>) {
        let auto_config_state = self.get_state_ref();
        Self::set_current_wpad_script_internal(auto_config_state, wpad);
    }

    fn set_current_wpad_script_internal(auto_config_state: Arc<Mutex<AutoConfigState>>, wpad: Option<String>) {
        let mut state = auto_config_state.lock().expect("issue locking state");
        *state = if let Some(ref script) = wpad {
            match pacparser::parse_pac_string(script) {
                Ok(..) => AutoConfigState::PAC,
                Err(..) => AutoConfigState::Direct,
            }
        } else {
            AutoConfigState::Direct
        };
        info!("AutoConfigState is now {:?}", *state)
    }
}
