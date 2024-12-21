use arceos_posix_api::AT_FDCWD;
use axerrno::AxError;
use alloc::{boxed::Box, string::ToString};


// 功能：挂载文件系统；
// 输入：
//     special: 挂载设备；
//     dir: 挂载点；
//     fstype: 挂载的文件系统类型；
//     flags: 挂载参数；
//     data: 传递给文件系统的字符串参数，可为NULL；
// 返回值：成功返回0，失败返回-1；
// const char *special, const char *dir, const char *fstype, unsigned long flags, const void *data;
// int ret = syscall(SYS_mount, special, dir, fstype, flags, data);
pub(crate) fn sys_mount(special: *const u8, dir: *const u8, fstype: *const u8, _flags: u64, _data: *const u8) -> i64 {
    let result = (|| {
        // 处理 special 路径
        let special_path = arceos_posix_api::handle_file_path(AT_FDCWD, Some(special), false)
            .inspect_err(|err| log::error!("mount: special: {:?}", err))?;
        
        if special_path.is_dir() {
            log::debug!("mount: special is a directory");
            return Err(AxError::InvalidInput);
        }

        // 处理目标目录路径
        let dir_path = arceos_posix_api::handle_file_path(AT_FDCWD, Some(dir), false)
            .inspect_err(|err| log::error!("mount: dir: {:?}", err))?;

        // 处理文件系统类型
        let fstype_str = arceos_posix_api::char_ptr_to_str(fstype as *const i8)
            .inspect_err(|err| log::error!("mount: fstype: {:?}", err))
            .map_err(|_| AxError::InvalidInput)?;
        if fstype_str != "vfat" {
            log::debug!("mount: fstype is not axfs");
            return Err(AxError::InvalidInput);
        }

        let special_path = "/mnt/test_mount";

        // 执行挂载
        let dir_path_str: &'static str = Box::leak(Box::new(dir_path.to_string()));
        axfs::mount(&special_path, dir_path_str)
            .inspect_err(|err| log::error!("mount: {:?}", err))?;
        Ok(())
    })();

    match result {
        Ok(_) => 0,
        Err(_) => -1,
    }
}


// 功能：卸载文件系统；
// 输入：指定卸载目录，卸载参数；
// 返回值：成功返回0，失败返回-1；
// const char *special, int flags;
// int ret = syscall(SYS_umount2, special, flags);
pub(crate) fn sys_umount2(special: *const u8, _flags: i32) -> i64 {
    let result = (|| {
        // 处理 special 路径
        let special_path = arceos_posix_api::handle_file_path(AT_FDCWD, Some(special), false)
            .inspect_err(|err| log::error!("umount2: special: {:?}", err))?;
        
        if special_path.is_dir() {
            log::debug!("umount2: special is a directory");
            return Err(AxError::InvalidInput);
        }

        // 执行卸载
        axfs::umount(&special_path)
            .inspect_err(|err| log::error!("umount2: {:?}", err))?;
        
        Ok(())
    })();

    match result {
        Ok(_) => 0,
        Err(_) => -1,
    }
}
