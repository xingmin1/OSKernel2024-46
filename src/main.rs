#![no_std]
#![no_main]
#![doc = include_str!("../README.md")]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate axstd;

#[rustfmt::skip]
mod config {
    include!(concat!(env!("OUT_DIR"), "/uspace_config.rs"));
}
mod loader;
mod mm;
mod syscall_imp;
mod task;

use alloc::sync::Arc;

use axhal::arch::UspaceContext;
use axsync::Mutex;

static VFAT12_IMG: &'static [u8] = include_bytes!("../vfat12.img");

const JUNIOR: &[&str] = &[
    "brk", "chdir", "clone", "close", "dup2", "dup", "execve", "exit", "fork", "fstat", "getcwd",
    "getdents", "getpid", "getppid", "gettimeofday", "mkdir_", "mmap", "mount", "munmap", "openat",
    "open", "pipe", "read", "sleep", "times", "umount", "uname", "unlink", "wait", "waitpid", "write", "yield"
];
// const JUNIOR: &[&str] = &["clone"];

#[no_mangle]
fn main() {
    // let testcases = option_env!("AX_TESTCASES_LIST")
    // .unwrap_or_else(|| "Please specify the testcases list by making user_apps")
    // .split(',')
    // .filter(|&x| !x.is_empty());

    // 为mount和umount测例准备 FAT12 文件系统镜像
    let _ = axfs::fops::File::open(
        "/vda2",
        &axfs::fops::OpenOptions::new()
            .set_crate(true, true)
            .set_read(true)
            .set_write(true),
    )
    .inspect_err(|err| debug!("Failed to open /vda2: {:?}", err))
    .and_then(|mut file| file.write(VFAT12_IMG))
    .inspect_err(|err| debug!("Failed to write /dev/vda2: {:?}", err));

    // 加载并运行测试用例
    let testcases = JUNIOR;
    for testcase in testcases {
        info!("Running testcase: {}", testcase);
        let (entry_vaddr, ustack_top, uspace) = mm::load_user_app(testcase).unwrap();
        let user_task = task::spawn_user_task(
            Arc::new(Mutex::new(uspace)),
            UspaceContext::new(entry_vaddr.into(), ustack_top, 2333),
        );
        let exit_code = user_task.join();
        info!("User task {} exited with code: {:?}", testcase, exit_code);
    }
}
