use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use slog::FilterLevel;

use crate::pacparser::ProxySuggestion;

/// Run a simple HTTP proxy on localhost that uses a wpad.dat to decide how to connect to the
/// actual server
#[derive(Debug, StructOpt)]
#[structopt(name = "pac4cli", rename_all="kebab")]
pub struct Options {
    /// Path to configuration file
    #[structopt(short, long)]
    pub config: Option<PathBuf>,

    /// Port to listen on
    #[structopt(short, long)]
    pub port: u16,

    /// Forward traffic according to PROXY STRING, e.g. DIRECT or PROXY <proxy>
    #[structopt(short, long)]
    pub force_proxy: Option<ProxySuggestion>,

    /// Forward traffic according to a wpad.dat at this URL
    #[structopt(long)]
    pub force_wpad_url: Option<String>,

    /// Logging verbosity
    #[structopt(long, parse(try_from_str = try_parse_filter_level))]
    pub loglevel: FilterLevel,

    /// Log through systemd
    #[structopt(long)]
    pub systemd: bool,
}

fn try_parse_filter_level(s: &str) -> Result<FilterLevel, String> {
    match FilterLevel::from_str(s) {
        Ok(level) => Ok(level),
        Err(()) => Err("Uknown loglevel".to_string()),
    }
}

