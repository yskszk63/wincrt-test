use std::io::{self, Error};
//use std::ffi::OsStr;
use std::ffi::CStr;
use std::ffi::c_void;
use std::os::raw::{c_int, c_uint, c_char};
use std::ffi::OsString;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
//use std::process::{Command, Stdio};
//use std::process::Command;
use std::mem;
use std::ptr;

use bindings::Windows::Win32::System::Pipes::CreatePipe;
use bindings::Windows::Win32::Security::SECURITY_ATTRIBUTES;
use bindings::Windows::Win32::Foundation::{HANDLE, CloseHandle};

mod bindings {
    windows::include_bindings!();
}

//memo
//https://stackoverflow.com/questions/34504970/non-blocking-read-on-os-pipe-on-windows


//https://www.rpi.edu/dept/cis/software/g77-mingw32/include/fcntl.h
const _O_BINARY: c_int = 0x8000;
//const _O_TEXT: c_int = 0x4000;
const O_NOINHERIT: c_int = 0x0080;
const _O_RDONLY: c_int = 0;
//https://www.rpi.edu/dept/cis/software/g77-mingw32/include/process.h
const _P_NOWAIT: c_int = 1;

extern "C" {
    //fn _putws(str: *const u16);
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/pipe?view=msvc-160
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
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/spawnv-wspawnv?view=msvc-160
    fn _spawnv(mode: c_int, cmdname: *const c_char, argv: *const *const c_char) -> isize;
    fn _wspawnv(mode: c_int, cmdname: *const u16, argv: *const *const u16) -> isize;
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/cwait?view=msvc-160
    fn _cwait(termstat: *mut c_int, prochandle: isize, action: c_int) -> isize;
    // https://docs.microsoft.com/ja-jp/cpp/c-runtime-library/reference/set-invalid-parameter-handler-set-thread-local-invalid-parameter-handler?view=msvc-160
    fn _set_invalid_parameter_handler(newp: *const c_void) -> *const c_void;
}

pub extern "C" fn my_invalid_paratemer(expression: *const u16, function_name: *const u16, file_name: *const u16, line_number: c_uint, _: isize) {
    panic!("_invalid_parameter")
}

#[derive(Debug)]
struct FileDescriptor(c_int);

impl FileDescriptor {
    unsafe fn from(raw: c_int) -> Self {
        Self(raw)
    }

    fn dup(&self) -> io::Result<FileDescriptor> {
        let ret = unsafe {
            _dup(self.0)
        };
        if ret < 0 {
            return Err(io::Error::last_os_error().into())
        }

        return Ok(Self(ret))
    }

    fn dup2(&self, no: c_int) -> io::Result<FileDescriptor> {
        let ret = unsafe {
            _dup2(self.0, no)
        };
        if ret < 0 {
            return Err(io::Error::last_os_error().into())
        }

        return Ok(Self(no))
    }
}

impl Drop for FileDescriptor {
    fn drop(&mut self) {
        unsafe {
            _close(self.0)
        };
    }
}

fn create_pipe() -> windows::Result<(HANDLE, HANDLE)> {
    let mut read_handle = HANDLE::default();
    let mut write_handle = HANDLE::default();
    let mut sec = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: false.into(),
    };

    unsafe {
        CreatePipe(
            &mut read_handle as *mut _,
            &mut write_handle as *mut _,
            &mut sec as *mut _,
            512,
        )
    }.ok()?;

    Ok((read_handle, write_handle))
}

fn into_fd(handle: HANDLE) -> anyhow::Result<FileDescriptor> {
    let HANDLE(raw) = handle;
    let fd = unsafe {
        _open_osfhandle(raw, _O_RDONLY)
    };
    if fd < 0 {
        return Err(io::Error::last_os_error().into())
    }

    Ok(unsafe {
        FileDescriptor::from(fd)
    })
}

fn swap_fd_with<E, R, F>(fd: FileDescriptor, no: c_int, fun: F) -> Result<R, E>
where F: FnOnce(FileDescriptor) -> Result<R, E>, E: From<io::Error> {
    let dup = fd.dup()?;

    // FIXME NOINHERIT?
    // dup no & close no
    let backup = unsafe { FileDescriptor::from(no) }.dup();

    let newfd = dup.dup2(no)?;
    drop(dup);

    let result = fun(newfd);

    if let Ok(backup) = backup {
        backup.dup2(no)?;
    };
    result
}

#[derive(Debug)]
struct CrtChild(isize);

impl CrtChild {
    fn wait(&mut self) -> io::Result<c_int> {
        let mut exitcode = c_int::default();
        let ret = unsafe {
            _cwait(&mut exitcode as *mut _, self.0, 0)
        };
        if ret < 0 {
            return Err(io::Error::last_os_error())
        }

        Ok(exitcode)
    }
}

fn crt_spawn<C>(program: C) -> io::Result<CrtChild> where C: Into<OsString> {
    let program = program.into();
    let mut program: Vec<u16> = {
        #[cfg(windows)]
        {
            program.encode_wide().collect()
        }

        #[cfg(not(windows))]
        {
            vec![]
        }
    };
    program.push(0);

    let args = [ program.as_ptr(), ptr::null() ];
    let child = unsafe {
        _wspawnv(_P_NOWAIT, program.as_ptr(), args.as_ptr())
    };
    if child < 0 {
        return Err(io::Error::last_os_error())
    }

    Ok(CrtChild(child))
}

fn main() -> anyhow::Result<()> {
    unsafe {
        _set_invalid_parameter_handler(my_invalid_paratemer as *const _);
    }

    let (r, w) = create_pipe()?;
    println!("pipe created.");
    let r = into_fd(r)?;
    println!("convert handle into fd.");
    let mut child = swap_fd_with(r, 3, move |fd| {
        println!("swap ok.");
        let child = crt_spawn("./target/debug/child")?;
        println!("spawned");
        drop(fd);

        Result::<_, anyhow::Error>::Ok(child)
    })?;

    unsafe {
        CloseHandle(w)
    }.ok()?;
    println!("closed");

    let exitcode = child.wait()?;
    println!("DONE {}", exitcode);

    Ok(())
}

#[allow(unused)]
fn main2() -> windows::Result<()> {
    let mut read_handle = HANDLE::default();
    let mut write_handle = HANDLE::default();
    let mut sec = SECURITY_ATTRIBUTES {
        nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: false.into(),
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

    assert_eq!(3, fd); // FIXME

    /*
    let mut child = Command::new("./target/debug/child")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    */
    let cmdname = CStr::from_bytes_with_nul(b"./target/debug/child\0").unwrap();
    let args = [ cmdname.as_ptr(), ptr::null() ];
    let child = unsafe {
        _spawnv(_P_NOWAIT, cmdname.as_ptr(), args.as_ptr())
    };
    if child < 0 {
        panic!("{}", io::Error::last_os_error())
    }

    unsafe {
        CloseHandle(read_handle)
    }.ok()?;

    unsafe {
        CloseHandle(write_handle)
    }.ok()?;

    let mut exitcode = c_int::default();
    let ret = unsafe {
        _cwait(&mut exitcode as *mut _, child, 0)
    };
    if ret < 0 {
        panic!("{}", io::Error::last_os_error())
    }
    //let exitcode = child.wait().unwrap();
    println!("{:?}", exitcode);

    Ok(())
}

#[allow(unused)]
fn main3() -> io::Result<()> {
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

    let cmdname = CStr::from_bytes_with_nul(b"./target/debug/child\0").unwrap();
    let args = [ cmdname.as_ptr(), ptr::null() ];
    let child = unsafe {
        _spawnv(_P_NOWAIT, cmdname.as_ptr(), args.as_ptr())
    };
    if child < 0 {
        panic!("{}", io::Error::last_os_error())
    }

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

    let ret = unsafe { _close(w) };
    if ret != 0 {
        return Err(Error::last_os_error())
    }

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

    let mut exitcode = c_int::default();
    let ret = unsafe {
        _cwait(&mut exitcode as *mut _, child, 0)
    };
    if ret < 0 {
        panic!("{}", io::Error::last_os_error())
    }
    println!("{:?}", exitcode);
    Ok(())
}
