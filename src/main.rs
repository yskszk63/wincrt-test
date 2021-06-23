use std::io::{self, Error};
//use std::ffi::OsStr;
use std::ffi::c_void;
use std::os::raw::{c_int, c_uint};
//use std::os::windows::ffi::OsStrExt;
use std::process::{Command, Stdio};
use std::mem;
use std::ptr;

use bindings::Windows::Win32::System::Pipes::CreatePipe;
use bindings::Windows::Win32::Security::SECURITY_ATTRIBUTES;
use bindings::Windows::Win32::Foundation::{HANDLE, BOOL};

mod bindings {
    windows::include_bindings!();
}

//https://www.rpi.edu/dept/cis/software/g77-mingw32/include/fcntl.h
const _O_BINARY: c_int = 0x8000;
//const _O_TEXT: c_int = 0x4000;
const O_NOINHERIT: c_int = 0x0080;
const _O_RDONLY: c_int = 0;

extern "C" {
    //fn _putws(str: *const u16);
    fn _pipe(pfds: *mut c_int, psize: c_uint, textmode: c_int) -> c_int;
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/write?view=msvc-160
    fn _write(fd: c_int, buffer: *const c_void, count: c_uint) -> c_int;
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/read?view=msvc-160
    fn _read(fd: c_int, buffer: *mut c_void, buffer_size: c_uint) -> c_int;
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/dup-dup2?view=msvc-160
    fn _dup(fd: c_int) -> c_int;
    fn _dup2(fd1: c_int, fd2: c_int) -> c_int;
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/close?view=msvc-160
    fn _close(fd: c_int) -> c_int;
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/open-osfhandle?view=msvc-160
    fn _open_osfhandle(osfhandle: isize, flags: c_int) -> c_int;
}

fn main() -> windows::Result<()> {
    let mut read_handle = HANDLE::default();
    let mut write_handle = HANDLE::default();
    let mut sec = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    unsafe {
        CreatePipe(
            &mut read_handle as *mut _,
            &mut write_handle as *mut _,
            &mut sec as *mut _,
            512,
        )
    }.ok()?;

    let HANDLE(raw) = read_handle;
    let fd = unsafe {
        _open_osfhandle(raw, _O_RDONLY)
    };
    if fd < 0 {
        panic!("{}", io::Error::last_os_error())
    }

    println!("{:?} {:?} {}", read_handle, write_handle, fd);

    Ok(())
}

fn main2() -> io::Result<()> {
    /*
    let greet = OsStr::new("Hello, World!");
    let greet = greet.encode_wide().collect::<Vec<_>>();
    unsafe {
        _putws(greet.as_ptr());
    }
    */

    let mut pfds = [0, 0];
    let ret = unsafe {
        _pipe(pfds.as_mut_ptr(), 512, _O_BINARY | O_NOINHERIT)
    };
    if ret != 0 {
        return Err(Error::last_os_error())
    }
    let [r, w] = pfds;
    println!("{} {}", r, w);

    let tfd = unsafe { _dup(r) };
    if tfd < 0 {
        return Err(Error::last_os_error())
    }
    let ret = unsafe { _dup2(tfd, r) };
    if ret < 0 {
        return Err(Error::last_os_error())
    }
    let ret = unsafe { _close(tfd) };
    if ret < 0 {
        return Err(Error::last_os_error())
    }

    let mut child = Command::new("./target/debug/child")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();

    let ret = unsafe { _close(r) };
    if ret != 0 {
        return Err(Error::last_os_error())
    }

    let buf = b"hello";
    let ret = unsafe {
        _write(w, buf.as_ptr() as *const _, buf.len() as c_uint)
    };
    if ret < 0 {
        return Err(Error::last_os_error())
    }
    println!("wrote");

    /*
    let mut buf = [0; 32];
    let ret = unsafe {
        _read(r, buf.as_mut_ptr() as *mut _, buf.len() as c_uint)
    };
    if ret < 0 {
        return Err(Error::last_os_error())
    }
    println!("{}", String::from_utf8_lossy(&buf[..ret as usize]));
    */

    let exitcode = child.wait().unwrap();
    println!("{:?}", exitcode);
    Ok(())
}
