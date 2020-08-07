use std::str::FromStr;
use std::string::ParseError;
use std::sync;

use async_trait::async_trait;
use reqwest;

use crate::options;
use crate::pacparser;
use crate::wpad;

#[derive(Clone, Debug)]
pub struct WPADInfo {
    pub wpad_option: Option<String>,
    pub domains: Vec<String>,
}

#[async_trait]
pub trait NetworkEnvironment {
    async fn get_wpad_info(&self) -> Result<WPADInfo, ()>;
}

#[derive(Clone, Debug)]
pub enum ProxySuggestion {
    Direct,
    Proxy { host: String, port: Option<u16> },
}

impl FromStr for ProxySuggestion {
    type Err = ParseError;

    fn from_str(suggestion: &str) -> Result<Self, Self::Err> {
        if suggestion == "DIRECT" {
            Ok(ProxySuggestion::Direct)
        } else if suggestion.starts_with("PROXY ") {
            let mut parts = suggestion[6..].split(":");
            let host = String::from(parts.next().unwrap());
            let port = match parts.next() {
                None => None,
                Some(p) => Some(p.parse::<u16>().expect("invalid port")),
            };
            Ok(ProxySuggestion::Proxy { host, port })
        } else {
            // TODO: error instead
            Ok(ProxySuggestion::Direct)
        }
    }
}

fn find_proxy_suggestions(url: &str, host: &str) -> Vec<ProxySuggestion> {
    return pacparser::find_proxy(&url, &host)
        .split(";")
        .map(|s| {
            s.parse::<ProxySuggestion>()
                .expect("failed to parse proxy suggestion")
        })
        .collect();
}

#[derive(Debug, Clone)]
pub enum ProxyResolutionBehavior {
    Static(ProxySuggestion),
    WPAD(String),
}

#[derive(Debug)]
pub struct ProxyResolver<T: NetworkEnvironment> {
    flags: options::CmdLineOptions,
    behavior: sync::RwLock<ProxyResolutionBehavior>,
    network_env: T,
}

impl<T: NetworkEnvironment> ProxyResolver<T> {
    pub async fn load(network_env: T, flags: options::CmdLineOptions) -> Self {
        let configured_behavior = Self::reload_behavior(&network_env, &flags).await;
        let behavior = if let ProxyResolutionBehavior::WPAD(script) = configured_behavior {
            match pacparser::parse_pac_string(&script) {
                Ok(..) => ProxyResolutionBehavior::WPAD(script),
                Err(..) => ProxyResolutionBehavior::Static(ProxySuggestion::Direct),
            }
        } else {
            configured_behavior
        };
        Self {
            network_env,
            flags,
            behavior: sync::RwLock::new(behavior),
        }
    }

    pub async fn reload(&self) {
        let configured_behavior = Self::reload_behavior(&self.network_env, &self.flags).await;
        let mut behavior = self.behavior.write().unwrap();
        *behavior = if let ProxyResolutionBehavior::WPAD(script) = configured_behavior {
            match pacparser::parse_pac_string(&script) {
                Ok(..) => ProxyResolutionBehavior::WPAD(script),
                Err(..) => ProxyResolutionBehavior::Static(ProxySuggestion::Direct),
            }
        } else {
            configured_behavior
        }
    }

    pub fn find_proxy(&self, url: &str, host: &str) -> ProxySuggestion {
        // Take the write() lock because I'm not sure pacparser is thread-safe
        let behavior = self.behavior.write().unwrap();
        match &*behavior {
            ProxyResolutionBehavior::Static(proxy_suggestion) => proxy_suggestion.clone(),
            // TODO: try all instead of just the first
            ProxyResolutionBehavior::WPAD(_) => find_proxy_suggestions(url, host).remove(0),
        }
    }

    async fn reload_behavior(
        network_env: &T,
        flags: &options::CmdLineOptions,
    ) -> ProxyResolutionBehavior {
        let options = options::Options::load(&flags);
        match options.force_proxy {
            Some(proxy_suggestion) => ProxyResolutionBehavior::Static(proxy_suggestion),
            None => {
                let urls = match options.wpad_url {
                    Some(ref url) => [url.clone()].to_vec(),
                    None => Self::get_wpad_urls(&network_env).await.unwrap(),
                };
                let maybe_wpad_script = wpad::retrieve_first_working_url(urls).await.unwrap();
                match maybe_wpad_script {
                    None => ProxyResolutionBehavior::Static(ProxySuggestion::Direct),
                    Some(wpad_script) => ProxyResolutionBehavior::WPAD(wpad_script),
                }
            }
        }
    }
    async fn get_wpad_urls(network_env: &T) -> Result<Vec<String>, ()> {
        let info = network_env.get_wpad_info().await?;
        info!("Found network information: {:?}", info);
        let url_strings = match info.wpad_option {
            None => info
                .domains
                .iter()
                .map(|d| format!("http://wpad.{}/wpad.dat", d))
                .collect(),
            Some(url) => [url].to_vec(),
        };
        Ok(url_strings)
    }
}

async fn retrieve_first_working_url(urls: Vec<String>) -> Result<Option<String>, ()> {
    for url in urls {
        match reqwest::get(&url).await {
            Ok(res) => {
                if res.status() != reqwest::StatusCode::OK {
                    // continue
                } else {
                    let wpad_script = res.text().await.unwrap();
                    trace!("wpad script: {}", wpad_script);
                    return Ok(Some(wpad_script));
                }
            }
            Err(err) => {
                // this error is expected, as we're just sending requests
                // to random wpad.<domain> hosts that don't even exist
                // in most networks
                info!("No wpad configuration found: {:?}", err);
            }
        }
    }
    return Ok(None);
}
