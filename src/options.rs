use ini::Ini;
use slog::FilterLevel;
use std::path;
use std::str::FromStr;
use structopt::StructOpt;

use crate::pacparser::ProxySuggestion;

/// Run a simple HTTP proxy on localhost that uses a wpad.dat to decide how to connect to the
/// actual server
#[derive(Debug, StructOpt, Clone)]
#[structopt(name = "pac4cli", rename_all = "kebab")]
pub struct CmdLineOptions {
    /// Path to configuration file
    #[structopt(short, long)]
    pub config: Option<path::PathBuf>,

    /// Port to listen on
    #[structopt(short, long)]
    pub port: u16,

    /// Forward traffic according to PROXY STRING, e.g. DIRECT or PROXY <proxy>
    #[structopt(short = "F", long)]
    pub force_proxy: Option<ProxySuggestion>,

    /// Forward traffic according to a wpad.dat at this URL
    #[structopt(long)]
    pub wpad_url: Option<String>,

    /// Logging verbosity
    #[structopt(long, default_value="INFO", parse(try_from_str = try_parse_filter_level))]
    pub loglevel: FilterLevel,

    /// Log through systemd
    #[structopt(long)]
    pub systemd: bool,
}

// FilterLevel::from_str does not have a string representation of the
// error, which is needed for using it in command line parsing.
// This simple wrapper takes care of that.
fn try_parse_filter_level(s: &str) -> Result<FilterLevel, String> {
    match FilterLevel::from_str(s) {
        Ok(level) => Ok(level),
        Err(()) => Err("Uknown loglevel".to_string()),
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub port: u16,
    pub force_proxy: Option<ProxySuggestion>,
    pub wpad_url: Option<String>,
    pub loglevel: FilterLevel,
    pub systemd: bool,
}

impl Options {
    pub fn load(flags: &CmdLineOptions) -> Self {
        let mut wpad_url = flags.wpad_url.clone();
        if let Some(file) = flags.config.clone() {
            info!("Loading configuration file {:?}", file);
            let conf = Ini::load_from_file(file).expect("Failed to load config file");
            if let Some(section) = conf.section(Some("wpad".to_owned())) {
                if let Some(url) = section.get("url") {
                    wpad_url = Some(url.clone())
                }
            }
        }
        Options {
            port: flags.port,
            force_proxy: flags.force_proxy.clone(),
            wpad_url: wpad_url,
            loglevel: flags.loglevel,
            systemd: flags.systemd,
        }
    }
}
