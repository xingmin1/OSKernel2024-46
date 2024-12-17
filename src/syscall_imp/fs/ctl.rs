use core::ffi::c_void;
use axhal::paging::MappingFlags;
use axtask::{current, TaskExtRef};
use memory_addr::VirtAddrRange;

use crate::syscall_body;

/// The ioctl() system call manipulates the underlying device parameters
/// of special files.
///
/// # Arguments
/// * `fd` - The file descriptor
/// * `op` - The request code. It is of type unsigned long in glibc and BSD,
/// and of type int in musl and other UNIX systems.
/// * `argp` - The argument to the request. It is a pointer to a memory location
pub(crate) fn sys_ioctl(_fd: i32, _op: usize, _argp: *mut c_void) -> i32 {
    syscall_body!(sys_ioctl, {
        warn!("Unimplemented syscall: SYS_IOCTL");
        Ok(0)
    })
}

/// 获取当前工作目录，返回一个包含工作目录的可变切片。
/// # 参数
/// * `buf` - 提供的缓冲区，可为 `NULL`，表示需要分配缓冲区。
/// * `size` - 缓冲区大小。
/// 
/// # 返回值
/// 成功时返回一个包含工作目录的切片；失败时返回 `NULL`。
pub fn sys_getcwd(buf: *mut u8, size: usize) -> isize {
    let cwd = match axfs::api::current_dir() {
        Ok(dir) => dir,
        Err(err) => {
            warn!("Failed to get current directory: {:?}", err);
            return core::ptr::null::<u8>() as isize;
        }
    };
    let cwd_len = cwd.len();

    if size <= cwd_len {
        return core::ptr::null::<u8>() as isize;
    }

    // 动态分配内存（如果 `buf` 为 `null`）
    let buf = if buf.is_null() {
        match allocate_user_buffer(size) {
            Some(addr) => addr,
            None => {
                warn!("Failed to allocate memory for getcwd");
                return core::ptr::null::<u8>() as isize;
            }
        }
    } else {
        buf
    };

    // 将当前工作目录写入缓冲区
    unsafe {
        let dst = core::slice::from_raw_parts_mut(buf, size);
        dst[..cwd_len].copy_from_slice(cwd.as_bytes());
        dst[cwd_len] = 0; // 添加 null 终止符
    }

    buf as isize
}

// 动态分配用户缓冲区
fn allocate_user_buffer(size: usize) -> Option<*mut u8> {
    let current_task = current();
    let curr_ext = current_task.task_ext();
    let mut aspace = curr_ext.aspace.lock();

    // 查找用户地址空间中的空闲区域
    let addr = aspace.find_free_area(
        aspace.base(),
        size,
        VirtAddrRange::new(aspace.base(), aspace.end()),
    )?;

    // 分配内存并映射到地址空间
    let flags = MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER;
    aspace.map_alloc(addr, size, flags, true).ok()?;

    Some(addr.as_usize() as *mut u8)
}

pub(crate) fn sys_dup(fd: i32) -> i32 {
    arceos_posix_api::get_file_like(fd)
        .and_then(|f| arceos_posix_api::add_file_like(f))
        .unwrap_or_else(|err| {
            warn!("Failed to duplicate file descriptor: {:?}", err);
            -1
        })
}

pub(crate) fn sys_dup3(old_fd: i32, new_fd: i32, flags: i32) -> i32 {
    if flags != 0 {
        warn!("Unsupported flags: {}", flags);
    }

    match arceos_posix_api::sys_dup2(old_fd, new_fd) {
        ok @0.. => ok as _,
        _ => -1,
    }
}
