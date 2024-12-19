use core::ffi::c_void;
use alloc::string::ToString;
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

/// 将当前工作目录更改为指定路径。
/// 
/// # 参数
/// * `path` - 指向包含目标目录路径的以 null 结尾的字符串的指针
/// 
/// # 返回值
/// * 成功时返回 `0`
/// * 失败时返回 `-1`
pub(crate) fn sys_chdir(path: *const i8) -> i32 {
    let path = match arceos_posix_api::char_ptr_to_str(path) {
        Ok(path) => path,
        Err(err) => {
            warn!("Failed to convert path: {err:?}");
            return -1;
        }
    };

    axfs::api::set_current_dir(path)
        .map(|_| 0)
        .unwrap_or_else(|err| {
            warn!("Failed to change directory: {err:?}");
            -1
        })
}

/// 在给定的目录文件描述符相对路径下创建一个新目录。
/// 
/// # 参数
/// * `dirfd` - 目录文件描述符（-100 表示当前工作目录）
/// * `path` - 指向包含目录路径的以 null 结尾的字符串的指针
/// * `mode` - 目录权限（当前忽略）
/// 
/// # 返回值
/// * 成功时返回 `0`
/// * 失败时返回 `-1`
pub(crate) fn sys_mkdirat(dirfd: i32, path: *const i8, mode: u32) -> i32 {
    const AT_FDCWD: i32 = -100;

    let path = match arceos_posix_api::char_ptr_to_str(path) {
        Ok(path) => path,
        Err(err) => {
            warn!("Failed to convert path: {err:?}");
            return -1;
        }
    };

    if !path.starts_with('/') && dirfd != AT_FDCWD {
        warn!("Unsupported dirfd: {dirfd}");
        return -1;
    }

    if mode != 0 {
        info!("Directory mode {mode} is currently ignored");
    }

    axfs::api::create_dir(path)
        .map(|_| 0)
        .unwrap_or_else(|err| {
            warn!("Failed to create directory: {err:?}");
            -1
        })
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct DirEnt {
    d_ino: u64,      // 索引结点号
    d_off: i64,      // 到下一个dirent的偏移
    d_reclen: u16,   // 当前dirent的长度
    d_type: u8,      // 文件类型
    d_name: [u8; 0], // 文件名
}

#[allow(dead_code)]
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum FileType {
    /// 未知类型文件
    Unknown = 0,
    /// FIFO
    Fifo = 1,
    /// 字符设备
    Chr = 2,
    /// 目录
    Dir = 4,
    /// 块设备
    Blk = 6,
    /// 常规文件
    Reg = 8,
    /// 符号链接
    Lnk = 10,
    /// Socket
    Socket = 12,
    /// Whiteout
    Wht = 14,
}

impl From<axfs::api::FileType> for FileType {
    fn from(ft: axfs::api::FileType) -> Self {
        match ft {
            ft if ft.is_dir() => FileType::Dir,
            ft if ft.is_file() => FileType::Reg,
            _ => FileType::Unknown,
        }
    }
}

impl DirEnt {
    const FIXED_SIZE: usize = core::mem::size_of::<u64>() + core::mem::size_of::<i64>() + core::mem::size_of::<u16>() + core::mem::size_of::<u8>();
    
    fn new(ino: u64, off: i64, reclen: usize, file_type: FileType) -> Self {
        Self {
            d_ino: ino,
            d_off: off,
            d_reclen: reclen as u16,
            d_type: file_type as u8,
            d_name: [],
        }
    }
    
    unsafe fn write_name(&mut self, name: &[u8]) {
        core::ptr::copy_nonoverlapping(
            name.as_ptr(),
            self.d_name.as_mut_ptr(),
            name.len(),
        );
    }
}

// Directory buffer for getdents64 syscall
struct DirBuffer<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> DirBuffer<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, offset: 0 }
    }
    
    fn remaining_space(&self) -> usize {
        self.buf.len().saturating_sub(self.offset)
    }
    
    fn can_fit_entry(&self, entry_size: usize) -> bool {
        self.remaining_space() >= entry_size
    }
    
    unsafe fn write_entry(&mut self, dirent: DirEnt, name: &[u8]) -> Result<(), ()> {
        if !self.can_fit_entry(dirent.d_reclen as usize) {
            return Err(());
        }
        
        let entry_ptr = self.buf.as_mut_ptr().add(self.offset) as *mut DirEnt;
        entry_ptr.write(dirent);
        (*entry_ptr).write_name(name);
        
        self.offset += dirent.d_reclen as usize;
        Ok(())
    }
}

pub(crate) fn sys_getdents64(fd: i32, buf: *mut c_void, len: usize) -> isize {
    if len < DirEnt::FIXED_SIZE {
        warn!("Buffer size too small: {len}");
        return -1;
    }
    
    let current = current();
    if let Err(e) = current.task_ext().aspace.lock().alloc_for_lazy((buf as usize).into(), len) {
        warn!("Memory allocation failed: {:?}", e);
        return -1;
    }

    // 获取文件描述符对应的目录路径
    let path = match arceos_posix_api::Directory::from_fd(fd).map(|dir| dir.path().to_string()) {
        Ok(path) => path,
        Err(err) => {
            warn!("Invalid directory descriptor: {:?}", err);
            return -1;
        }
    };

    let mut buffer = unsafe { DirBuffer::new(core::slice::from_raw_parts_mut(buf as *mut u8, len)) };
    
    // 得到初始偏移量和目录项数量
    let (initial_offset, count) = unsafe {
        let mut buf_offset = 0;
        let mut count = 0;
        while buf_offset + DirEnt::FIXED_SIZE <= len {
            let dir_ent = *(buf.add(buf_offset) as *const DirEnt);
            if dir_ent.d_reclen == 0 { break; }
            
            buf_offset += dir_ent.d_reclen as usize;
            assert_eq!(dir_ent.d_off, buf_offset as i64);
            count += 1;
        }
        (buf_offset as i64, count)
    };

    // 读取目录项并写入缓冲区
    axfs::api::read_dir(&path)
        .map_err(|_| -1)
        .and_then(|entries| {
            let mut total_size = initial_offset as usize;
            let mut current_offset = initial_offset;
            
            for entry in entries.flatten().skip(count) {
                let mut name = entry.file_name();
                name.push('\0');
                let name_bytes = name.as_bytes();
                
                let entry_size = DirEnt::FIXED_SIZE + name_bytes.len();
                current_offset += entry_size as i64;
                
                let dirent = DirEnt::new(
                    1, 
                    current_offset,
                    entry_size,
                    FileType::from(entry.file_type()),
                );
                
                unsafe {
                    if buffer.write_entry(dirent, name_bytes).is_err() {
                        break;
                    }
                }
                
                total_size += entry_size;
            }
            
            // 添加终止目录项
            if total_size > 0 && buffer.can_fit_entry(DirEnt::FIXED_SIZE) {
                let terminal = DirEnt::new(1, current_offset, 0, FileType::Reg);
                unsafe { let _ = buffer.write_entry(terminal, &[]); }
            }
            
            Ok(total_size as isize)
        })
        .unwrap_or(-1)
}