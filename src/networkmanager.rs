use std::collections::HashMap;
use std::sync;
use std::time::Duration;

use async_trait::async_trait;
use dbus;
use dbus::arg::Variant;
use dbus::nonblock::{Proxy, SyncConnection};
use dbus::Path;
use dbus_tokio::connection;

use crate::wpad;

pub struct NetworkManager {
    dbus_conn: sync::Arc<SyncConnection>,
}

impl NetworkManager {
    pub fn new() -> Self {
        let (resource, dbus_conn) = connection::new_system_sync().unwrap();
        tokio::spawn(async {
            let err = resource.await;
            panic!("Lost connection to D-Bus: {}", err);
        });
        NetworkManager { dbus_conn }
    }

    async fn _get_wpad_info(&self) -> Result<wpad::WPADInfo, dbus::Error> {
        debug!("Finding active connections");
        let active_connections = get_list_of_paths(
            &self.dbus_conn,
            "/org/freedesktop/NetworkManager",
            "org.freedesktop.NetworkManager",
            "ActiveConnections",
        )
        .await?;

        debug!("received active connections: {:?}", active_connections);

        let mut wpad_info = wpad::WPADInfo {
            wpad_option: None,
            domains: Vec::new(),
        };

        for active_connection in active_connections {
            let config_path = get_path(
                &self.dbus_conn,
                active_connection.clone(),
                "org.freedesktop.NetworkManager.Connection.Active",
                "Dhcp4Config",
            )
            .await?;

            debug!("received config path: {:?}", config_path);
            let options = get_dict(
                &self.dbus_conn,
                config_path.clone(),
                "org.freedesktop.NetworkManager.DHCP4Config",
                "Options",
            )
            .await?;

            debug!("received dhcp4 options: {:?}", options);
            wpad_info.wpad_option = match options.get(&String::from("wpad")) {
                None => None,
                Some(s) => Some(s.0.clone()),
            };
            let config_path = get_path(
                &self.dbus_conn,
                active_connection.clone(),
                "org.freedesktop.NetworkManager.Connection.Active",
                "Ip4Config",
            )
            .await?;

            debug!("received config path: {:?}", config_path);
            let domains = get_list_of_strings(
                &self.dbus_conn,
                config_path.clone(),
                "org.freedesktop.NetworkManager.IP4Config",
                "Domains",
            )
            .await?;

            debug!("received domains: {:?}", domains);
            wpad_info.domains.extend(domains);
        }
        return Ok(wpad_info);
    }
}

#[async_trait]
impl wpad::NetworkEnvironment for NetworkManager {
    async fn get_wpad_info(&self) -> Result<wpad::WPADInfo, ()> {
        self._get_wpad_info().await.map_err(|_| ())
    }
}

const TIMEOUT: Duration = Duration::from_secs(2);

async fn get_list_of_paths<P>(
    dbus_conn: &SyncConnection,
    object_path: P,
    interface: &str,
    property: &str,
) -> Result<Vec<Path<'static>>, dbus::Error>
where
    P: Into<Path<'static>>,
{
    let proxy = Proxy::new(
        "org.freedesktop.NetworkManager",
        object_path,
        TIMEOUT,
        dbus_conn,
    );
    let (res,): (Variant<Vec<Path<'static>>>,) = proxy
        .method_call(
            "org.freedesktop.DBus.Properties",
            "Get",
            (interface, property),
        )
        .await?;
    return Ok(res.0);
}

async fn get_path<P>(
    dbus_conn: &SyncConnection,
    object_path: P,
    interface: &str,
    property: &str,
) -> Result<Path<'static>, dbus::Error>
where
    P: Into<Path<'static>>,
{
    let proxy = Proxy::new(
        "org.freedesktop.NetworkManager",
        object_path,
        TIMEOUT,
        dbus_conn,
    );
    let (res,): (Variant<Path<'static>>,) = proxy
        .method_call(
            "org.freedesktop.DBus.Properties",
            "Get",
            (interface, property),
        )
        .await?;
    return Ok(res.0);
}

async fn get_dict<P>(
    dbus_conn: &SyncConnection,
    object_path: P,
    interface: &str,
    property: &str,
) -> Result<HashMap<String, Variant<String>>, dbus::Error>
where
    P: Into<Path<'static>>,
{
    let proxy = Proxy::new(
        "org.freedesktop.NetworkManager",
        object_path,
        TIMEOUT,
        dbus_conn,
    );
    let (res,): (Variant<HashMap<String, Variant<String>>>,) = proxy
        .method_call(
            "org.freedesktop.DBus.Properties",
            "Get",
            (interface, property),
        )
        .await?;
    return Ok(res.0);
}

async fn get_list_of_strings<P>(
    dbus_conn: &SyncConnection,
    object_path: P,
    interface: &str,
    property: &str,
) -> Result<Vec<String>, dbus::Error>
where
    P: Into<Path<'static>>,
{
    let proxy = Proxy::new(
        "org.freedesktop.NetworkManager",
        object_path,
        TIMEOUT,
        dbus_conn,
    );
    let (res,): (Variant<Vec<String>>,) = proxy
        .method_call(
            "org.freedesktop.DBus.Properties",
            "Get",
            (interface, property),
        )
        .await?;
    return Ok(res.0);
}
