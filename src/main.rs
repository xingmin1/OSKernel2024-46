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
