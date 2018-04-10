extern crate libc;
use self::libc::c_char;
use std::ffi::CString;
use std::ffi::CStr;
use std::str::FromStr;
use std::string::ParseError;

#[link(name="pacparser")]
extern "C" {
    fn pacparser_init() -> i64;
    fn pacparser_parse_pac_string(pacstring: *const c_char) -> i64;
    fn pacparser_find_proxy(url: *const c_char, host: *const c_char) -> *const c_char;
    fn pacparser_cleanup();
}

pub fn init() -> Result<(), ()> {
    unsafe {
        return if pacparser_init() == 1 { Ok(()) } else { Err(()) }
    }
}

pub fn parse_pac_string(pacstring: String) -> Result<(),()> {
    unsafe {
        let cstr = CString::new(pacstring).unwrap();
        return if pacparser_parse_pac_string(cstr.as_ptr()) == 1 { Ok(()) } else { Err(()) }
    }
}

fn find_proxy(url: &str, host: &str) -> String {
    unsafe {
        let curl = CString::new(url).unwrap();
        let chost = CString::new(host).unwrap();
        let cres = CStr::from_ptr( pacparser_find_proxy(curl.as_ptr(), chost.as_ptr()) );
        return String::from( cres.to_str().unwrap() );
    }
}

#[derive(Clone)]
pub enum ProxySuggestion {
    Direct,
    Proxy {
        host: String,
        port: Option<u16>,
    }
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
                Some(p) => {
                    Some(p.parse::<u16>().expect("invalid port"))
                }
            };
            Ok(ProxySuggestion::Proxy { host, port })
        } else {
            // TODO: error instead
            Ok(ProxySuggestion::Direct)
        }
    }
}

pub fn find_proxy_suggestions(url: &str, host: &str) -> Vec<ProxySuggestion> {
    return find_proxy(&url, &host).split(";").map(|s| s.parse::<ProxySuggestion>().expect("failed to parse proxy suggestion")).collect();
}

pub fn cleanup() {
    unsafe {
        pacparser_cleanup();
    }
}
