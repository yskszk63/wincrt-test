use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

extern "C" {
    fn _putws(str: *const [u16]);
}

fn main() {
    let greet = OsStr::new("Hello, World!");
    let greet = greet.encode_wide().collect::<Vec<_>>();
    unsafe {
        _putws(greet.as_ptr());
    }
}
