extern crate libc;
use self::libc::c_char;
use std::ffi::CStr;

#[link(name="systemd")]
extern "C" {
    fn sd_notify(unset_environment: i64, state: *const c_char) -> i64;
}
pub fn notify_ready() {
    let state = CStr::from_bytes_with_nul(b"READY=1\0").unwrap();
    unsafe {
        sd_notify(0, state.as_ptr()); // ignore return value as recommended by manpage
    }
}
