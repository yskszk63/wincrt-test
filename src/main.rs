use std::io::{self, Error};
use std::ffi::OsStr;
use std::os::raw::{c_int, c_uint};
use std::os::windows::ffi::OsStrExt;

const O_BINARY: c_int = 0;

extern "C" {
    fn _putws(str: *const u16);
    fn _pipe(pfds: *mut c_int, psize: c_uint, textmode: c_int) -> c_int;
}

fn main() -> io::Result<()> {
    let greet = OsStr::new("Hello, World!");
    let greet = greet.encode_wide().collect::<Vec<_>>();
    unsafe {
        _putws(greet.as_ptr());
    }

    let mut pfds = [0, 0];
    let ret = unsafe {
        _pipe(pfds.as_mut_ptr(), 512, O_BINARY)
    };
    if ret != 0 {
        return Err(Error::last_os_error())
    }
    let [r, w] = pfds;
    println!("{} {}", r, w);

    Ok(())
}
