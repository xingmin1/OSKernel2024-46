use core::ffi::c_long;

use arceos_posix_api as api;
use axtask::{current, TaskExtRef};

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

#[repr(C)]
pub(crate) struct Tms {
    tms_utime: c_long,
    tms_stime: c_long,
    tms_cutime: c_long,
    tms_cstime: c_long,
}

/// 功能：获取进程时间；
/// 输入：tms结构体指针，用于获取保存当前进程的运行时间数据；
/// 返回值：成功返回已经过去的滴答数，失败返回-1;
pub(crate) fn sys_times(buf: *mut Tms) -> i32 {
    if buf.is_null() {
        return -1;
    }

    let (user_time, kernel_time) = current().task_ext().time_stat.lock().info();
    let mut children_user_time = 0;
    let mut children_kernel_time = 0;
    current()
        .task_ext()
        .children
        .lock()
        .iter()
        .filter(|child| child.state() == axtask::TaskState::Exited)
        .for_each(|child| {
            let (child_user_time, child_kernel_time) = child.task_ext().time_stat.lock().info();
            children_user_time += child_user_time;
            children_kernel_time += child_kernel_time;
        });
    let tms = Tms {
        tms_utime: user_time as c_long,
        tms_stime: kernel_time as c_long,
        tms_cutime: children_user_time as c_long,
        tms_cstime: children_kernel_time as c_long,
    };
    unsafe {
        *buf = tms;
    }
    axhal::time::current_ticks() as i32
}
