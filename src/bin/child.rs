use std::io::Error;
use std::ffi::c_void;
use std::os::raw::{c_int, c_uint};

extern "C" {
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/read?view=msvc-160
    fn _read(fd: c_int, buffer: *mut c_void, buffer_size: c_uint) -> c_int;
}

fn main() {
    println!("Im child.");
    eprintln!("Im child.");

    let mut buf = [0; 32];
    let mut n = 0;
    loop {
        println!("loop");
        let ret = unsafe {
            _read(3, buf[n..].as_mut_ptr() as *mut _, buf[n..].len() as c_uint)
        };
        println!("{}", ret);
        if ret < 0 {
            let err = Error::last_os_error();
            println!("{}", err);
            panic!("{}", err);
        }
        n += ret as usize;
        if ret == 0 {
            break
        }
    }
    println!("'{}'", String::from_utf8_lossy(&buf[..n]));

    println!("Done.");
}
