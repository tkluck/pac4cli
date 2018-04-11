use std::rc::Rc;
use std::collections::HashMap;

use dbus;
use dbus::{BusType,Connection,Message,Path};
use dbus::arg::Variant;
use tokio::prelude::*;
use tokio_core::reactor::Core;
use dbus_tokio::AConnection;

fn get_list_of_paths<P>(aconn: &AConnection, object_path: P, interface: &str, property: &str) -> Box<Future<Item=Vec<Path<'static>>, Error=dbus::Error>>
    where P: Into<Path<'static>> {
    let m = Message::new_method_call("org.freedesktop.NetworkManager", object_path, "org.freedesktop.DBus.Properties", "Get").unwrap().append2(interface, property);
    let method_call = aconn.method_call(m).unwrap()
        .map(|m| {
            let res : Variant<Vec<Path<'static>>> = m.get1().unwrap();
            res.0
        });
    return Box::new(method_call);
}

fn get_path<P>(aconn: &AConnection, object_path: P, interface: &str, property: &str) -> Box<Future<Item=Path<'static>, Error=dbus::Error>>
    where P: Into<Path<'static>> {
    let m = Message::new_method_call("org.freedesktop.NetworkManager", object_path, "org.freedesktop.DBus.Properties", "Get").unwrap().append2(interface, property);
    let method_call = aconn.method_call(m).unwrap()
        .map(|m| {
            let res : Variant<Path<'static>> = m.get1().unwrap();
            res.0
        });
    return Box::new(method_call);
}

fn get_dict<P>(aconn: &AConnection, object_path: P, interface: &str, property: &str) -> Box<Future<Item=HashMap<String,String>, Error=dbus::Error>>
    where P: Into<Path<'static>> {
    let m = Message::new_method_call("org.freedesktop.NetworkManager", object_path, "org.freedesktop.DBus.Properties", "Get").unwrap().append2(interface, property);
    let method_call = aconn.method_call(m).unwrap()
        .map(|m| {
            let res : Variant<HashMap<String,String>> = m.get1().unwrap();
            res.0
        });
    return Box::new(method_call);
}

fn get_list_of_strings<P>(aconn: &AConnection, object_path: P, interface: &str, property: &str) -> Box<Future<Item=Vec<String>, Error=dbus::Error>>
    where P: Into<Path<'static>> {
    let m = Message::new_method_call("org.freedesktop.NetworkManager", object_path, "org.freedesktop.DBus.Properties", "Get").unwrap().append2(interface, property);
    let method_call = aconn.method_call(m).unwrap()
        .map(|m| {
            let res : Variant<Vec<String>> = m.get1().unwrap();
            res.0
        });
    return Box::new(method_call);
}

enum State {
    Start,
    ReceiveActiveConnections {
        paths_future: Box<Future<Item=Vec<Path<'static>>,Error=dbus::Error>>,
    },
    LoopConnections,
    ReceiveDhcp4Config {
        dhcp4_config_future: Box<Future<Item=Path<'static>,Error=dbus::Error>>,
    },
    ReceiveDhcp4Options {
        dhcp4_options_future: Box<Future<Item=HashMap<String,String>,Error=dbus::Error>>
    },
    ReceiveIP4Config {
        ip4_config_future: Box<Future<Item=Path<'static>,Error=dbus::Error>>,
    },
    ReceiveDomain {
        domain_future: Box<Future<Item=Vec<String>,Error=dbus::Error>>,
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
    fn new(core: &mut Core) -> Self {
        let c = Rc::new(Connection::get_private(BusType::System).unwrap());
        let aconn = AConnection::new(c.clone(), core.handle()).unwrap();
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
                     let paths_future = get_list_of_paths(&self.aconn, "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager", "ActiveConnections");
                     State::ReceiveActiveConnections { paths_future }
                 }
                 State::ReceiveActiveConnections { ref mut paths_future } => {
                     let active_connections = try_ready!(paths_future.poll());
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
                     let dhcp4_options_future = get_dict(&self.aconn, config_path.clone(), "org.freedesktop.NetworkManager.DHCP4Config", "Options");
                     State::ReceiveDhcp4Options { dhcp4_options_future }
                 }
                 State::ReceiveDhcp4Options { ref mut dhcp4_options_future } => {
                     let options = try_ready!(dhcp4_options_future.poll());
                     println!("got options: {:?}", options);
                     self.wpad_info.wpad_option = options.get(&String::from("wpad")).cloned();

                     let ip4_config_future = get_path(&self.aconn, self.active_connections[self.ix].clone(), "org.freedesktop.NetworkManager.Connection.Active", "Ip4Config");
                     State::ReceiveIP4Config { ip4_config_future }
                 }
                 State::ReceiveIP4Config { ref mut ip4_config_future } => {
                     let config_path = try_ready!(ip4_config_future.poll());
                     let domain_future = get_list_of_strings(&self.aconn, config_path.clone(), "org.freedesktop.NetworkManager.IP4Config", "Domains");
                     State::ReceiveDomain { domain_future }
                 }
                 State::ReceiveDomain { ref mut domain_future } => {
                     let domains = try_ready!(domain_future.poll());
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

pub fn get_wpad_file() -> String {

    let mut core = Core::new().unwrap();
    let task = WPADDiscoverer::new(&mut core).map(|info| {
        println!("Found information: {:?}", info)
    }).map_err(|_| ());

    println!("Sending dbus call");
    core.run(task).unwrap();

    String::from("abc")
}
