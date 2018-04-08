extern crate libc;
use self::libc::c_char;
use std::ffi::CString;
use std::ffi::CStr;

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

pub fn find_proxy(url: &str, host: &str) -> String {
    unsafe {
        let curl = CString::new(url).unwrap();
        let chost = CString::new(host).unwrap();
        let cres = CStr::from_ptr( pacparser_find_proxy(curl.as_ptr(), chost.as_ptr()) );
        return String::from( cres.to_str().unwrap() );
    }
}
pub fn cleanup() {
    unsafe {
        pacparser_cleanup();
    }
}
