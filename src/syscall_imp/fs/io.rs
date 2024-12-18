use core::ffi::c_void;

use arceos_posix_api::{self as api, ctypes::mode_t};

pub(crate) fn sys_read(fd: i32, buf: *mut c_void, count: usize) -> isize {
    api::sys_read(fd, buf, count)
}

pub(crate) fn sys_write(fd: i32, buf: *const c_void, count: usize) -> isize {
    api::sys_write(fd, buf, count)
}

pub(crate) fn sys_writev(fd: i32, iov: *const api::ctypes::iovec, iocnt: i32) -> isize {
    unsafe { api::sys_writev(fd, iov, iocnt) }
}

pub(crate) fn sys_pipe2(fds: *mut i32, flags: i32) -> isize {
    if flags != 0 {
        warn!("sys_pipe2: flags are not supported, ignoring");
    }

    let fds = match unsafe { fds.as_mut() } {
        Some(ptr) => unsafe { core::slice::from_raw_parts_mut(ptr, 2) },
        None => {
            error!("sys_pipe2: invalid fds pointer");
            return -1;
        },
    };

    match api::sys_pipe(fds) {
        0 => 0,
        err => {
            error!("sys_pipe2: failed to create pipe, error code {}", err);
            -1 
        },
    }
}

pub(crate) fn sys_close(fd: i32) -> isize {
    match api::sys_close(fd) {
        0 => 0,
        err => {
            error!("sys_close: failed to close file descriptor, error code {}", err);
            -1
        },
    }
}

pub(crate) fn sys_openat(dirfd: i32, path: *const i8, flags: i32, mode: mode_t) -> isize {
    api::sys_openat(dirfd, path, flags, mode) as isize
}