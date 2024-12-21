use arceos_posix_api as api;

pub(crate) fn sys_clock_gettime(clock_id: i32, tp: *mut api::ctypes::timespec) -> i32 {
    unsafe { api::sys_clock_gettime(clock_id, tp) }
}

pub(crate) fn sys_gettimeofday(tp: *mut api::ctypes::timeval, _tzp: usize) -> i32 {
    let mut ts = api::ctypes::timespec::default();
    let ret = unsafe {
        api::sys_clock_gettime(
            api::ctypes::CLOCK_REALTIME as i32,
            &mut ts as *mut api::ctypes::timespec,
        )
    };
    if ret != 0 {
        return ret;
    }
    unsafe {
        (*tp).tv_sec = ts.tv_sec;
        (*tp).tv_usec = ts.tv_nsec / 1000;
    }
    0
}
