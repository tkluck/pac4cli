use std::collections::HashMap;
use std::sync::{Mutex,Arc};
use std::time::Duration;

use dbus;
use dbus::Path;
use dbus::nonblock::{Proxy, SyncConnection};
use dbus::arg::Variant;
use dbus_tokio::connection;
use reqwest;
use reqwest::StatusCode;

use crate::pacparser;

const TIMEOUT : Duration = Duration::from_secs(2);

async fn get_list_of_paths<P>(dbus_conn: &SyncConnection, object_path: P, interface: &str, property: &str) -> Result<Vec<Path<'static>>, dbus::Error>
    where P: Into<Path<'static>> {
    let proxy = Proxy::new("org.freedesktop.NetworkManager", object_path, TIMEOUT, dbus_conn);
    let (res,) : (Variant<Vec<Path<'static>>>,) = proxy.method_call("org.freedesktop.DBus.Properties", "Get", (interface, property)).await?;
    return Ok(res.0)
}

async fn get_path<P>(dbus_conn: &SyncConnection, object_path: P, interface: &str, property: &str) -> Result<Path<'static>, dbus::Error>
    where P: Into<Path<'static>> {
    let proxy = Proxy::new("org.freedesktop.NetworkManager", object_path, TIMEOUT, dbus_conn);
    let (res,) : (Variant<Path<'static>>,) = proxy.method_call("org.freedesktop.DBus.Properties", "Get", (interface, property)).await?;
    return Ok(res.0)
}

async fn get_dict<P>(dbus_conn: &SyncConnection, object_path: P, interface: &str, property: &str) -> Result<HashMap<String, Variant<String>>, dbus::Error>
    where P: Into<Path<'static>> {
    let proxy = Proxy::new("org.freedesktop.NetworkManager", object_path, TIMEOUT, dbus_conn);
    let (res,) : (Variant<HashMap<String, Variant<String>>>,) = proxy.method_call("org.freedesktop.DBus.Properties", "Get", (interface, property)).await?;
    return Ok(res.0)
}

async fn get_list_of_strings<P>(dbus_conn: &SyncConnection, object_path: P, interface: &str, property: &str) -> Result<Vec<String>, dbus::Error>
    where P: Into<Path<'static>> {
    let proxy = Proxy::new("org.freedesktop.NetworkManager", object_path, TIMEOUT, dbus_conn);
    let (res,) : (Variant<Vec<String>>,) = proxy.method_call("org.freedesktop.DBus.Properties", "Get", (interface, property)).await?;
    return Ok(res.0)
}

#[derive(Clone,Debug)]
struct WPADInfo {
    wpad_option: Option<String>,
    domains: Vec<String>,
}

async fn wpaddiscoverer(conn : &SyncConnection) -> Result<WPADInfo, dbus::Error> {
    debug!("Finding active connections");
    let active_connections = get_list_of_paths(&conn, "/org/freedesktop/NetworkManager", "org.freedesktop.NetworkManager", "ActiveConnections").await?;

    debug!("received active connections: {:?}", active_connections);

    let mut wpad_info = WPADInfo {
        wpad_option: None,
        domains: Vec::new(),
    };

    for active_connection in active_connections {
        let config_path = get_path(&conn, active_connection.clone(), "org.freedesktop.NetworkManager.Connection.Active", "Dhcp4Config").await?;

        debug!("received config path: {:?}", config_path);
        let options = get_dict(&conn, config_path.clone(), "org.freedesktop.NetworkManager.DHCP4Config", "Options").await?;

        debug!("received dhcp4 options: {:?}", options);
        wpad_info.wpad_option = match options.get(&String::from("wpad")) {
            None => None,
            Some(s) => Some(s.0.clone()),
        };
        let config_path = get_path(&conn, active_connection.clone(), "org.freedesktop.NetworkManager.Connection.Active", "Ip4Config").await?;

        debug!("received config path: {:?}", config_path);
        let domains = get_list_of_strings(&conn, config_path.clone(), "org.freedesktop.NetworkManager.IP4Config", "Domains").await?;

        debug!("received domains: {:?}", domains);
        wpad_info.domains.extend(domains);
    }

    return Ok(wpad_info)
}


pub async fn get_wpad_urls() -> Result<Vec<String>, ()> {
    // TODO: cache me
    let (resource, dbus_conn) = connection::new_system_sync().unwrap();
    tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });

    match wpaddiscoverer(&dbus_conn).await {
        Err(dbus_err) => {
            warn!("dbus error: {:?}", dbus_err);
            Err(())
        },
        Ok(info) => {
            info!("Found network information: {:?}", info);
            let url_strings = match info.wpad_option {
                None => info.domains.iter().map(|d| {
                    format!("http://wpad.{}/wpad.dat", d)
                }).collect(),
                Some(url) => [url].to_vec(),
            };
            Ok(url_strings)
        }
    }
}

pub async fn retrieve_first_working_url(urls: Vec<String>) -> Result<Option<String>,()> {

    for url in urls {
        match reqwest::get(&url).await {
            Ok(res) => {
                if res.status() != StatusCode::OK {
                    // continue
                } else {
                    let wpad_script = res.text().await.unwrap();
                    trace!("wpad script: {}", wpad_script);
                    return Ok(Some(wpad_script))
                }
            },
            Err(err) => {
                // this error is expected, as we're just sending requests
                // to random wpad.<domain> hosts that don't even exist
                // in most networks
                info!("No wpad configuration found: {:?}", err);
            }
        }
    }
    return Ok(None)
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
