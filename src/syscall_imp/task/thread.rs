use arceos_posix_api::{self as api};
use axtask::{current, TaskExtRef};
use num_enum::TryFromPrimitive;

use crate::{syscall_body, task::clone_task};

/// ARCH_PRCTL codes
///
/// It is only avaliable on x86_64, and is not convenient
/// to generate automatically via c_to_rust binding.
#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(i32)]
enum ArchPrctlCode {
    /// Set the GS segment base
    SetGs = 0x1001,
    /// Set the FS segment base
    SetFs = 0x1002,
    /// Get the FS segment base
    GetFs = 0x1003,
    /// Get the GS segment base
    GetGs = 0x1004,
    /// The setting of the flag manipulated by ARCH_SET_CPUID
    GetCpuid = 0x1011,
    /// Enable (addr != 0) or disable (addr == 0) the cpuid instruction for the calling thread.
    SetCpuid = 0x1012,
}

pub(crate) fn sys_getpid() -> i32 {
    current().task_ext().proc_id as i32
}

pub(crate) fn sys_gettid() -> i32 {
    api::sys_getpid()
}

pub(crate) fn sys_exit(status: i32) -> ! {
    let curr = current();
    let clear_child_tid = curr.task_ext().clear_child_tid() as *mut i32;
    if !clear_child_tid.is_null() {
        // TODO: check whether the address is valid
        unsafe {
            // TODO: Encapsulate all operations that access user-mode memory into a unified function
            *(clear_child_tid) = 0;
        }
        // TODO: wake up threads, which are blocked by futex, and waiting for the address pointed by clear_child_tid
    }
    axtask::exit(status);
}

/// # Arguments for riscv
/// * `flags` - usize
/// * `user_stack` - usize
/// * `ptid` - usize
/// * `tls` - usize
/// * `ctid` - usize
///
/// # Arguments for x86_64
/// * `flags` - usize
/// * `user_stack` - usize
/// * `ptid` - usize
/// * `ctid` - usize
/// * `tls` - usize
pub fn sys_clone(flags: usize, user_stack: usize, ptid_riscv: usize, tls_riscv: usize, ctid: usize) -> isize {
    let ptid    ;
    let tls     ;
    #[cfg(target_arch = "x86_64")]
    {
        ptid = tls_riscv;
        tls = ctid;
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        ptid = ptid_riscv;
        tls = tls_riscv;
    }

    let stack = if user_stack == 0 {
        None
    } else {
        Some(user_stack)
    };
    const SIGNAL_MASK: usize = 0x3f; // 0x3f = 0b111111
    if flags & SIGNAL_MASK != 0 {
        info!("Unsupported signal: 0x{:x}", flags & SIGNAL_MASK);
    }
    let clone_flags = flags & !SIGNAL_MASK;
    if clone_flags != 0 {
        info!("Unsupported clone flags: 0x{:x}", clone_flags);
    }

    if let Ok(new_task_id) = clone_task(flags, stack, ptid, tls, ctid) { 
       new_task_id as isize
    } else {
        -1
    }
}

pub(crate) fn sys_exit_group(status: i32) -> ! {
    warn!("Temporarily replace sys_exit_group with sys_exit");
    axtask::exit(status);
}

/// To set the clear_child_tid field in the task extended data.
///
/// The set_tid_address() always succeeds
pub(crate) fn sys_set_tid_address(tid_ptd: *const i32) -> isize {
    syscall_body!(sys_set_tid_address, {
        let curr = current();
        curr.task_ext().set_clear_child_tid(tid_ptd as _);
        Ok(curr.id().as_u64() as isize)
    })
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn sys_arch_prctl(code: i32, addr: u64) -> isize {
    use axerrno::LinuxError;
    syscall_body!(sys_arch_prctl, {
        match ArchPrctlCode::try_from(code) {
            // TODO: check the legality of the address
            Ok(ArchPrctlCode::SetFs) => {
                unsafe {
                    axhal::arch::write_thread_pointer(addr as usize);
                }
                Ok(0)
            }
            Ok(ArchPrctlCode::GetFs) => {
                unsafe {
                    *(addr as *mut u64) = axhal::arch::read_thread_pointer() as u64;
                }
                Ok(0)
            }
            Ok(ArchPrctlCode::SetGs) => {
                unsafe {
                    x86::msr::wrmsr(x86::msr::IA32_KERNEL_GSBASE, addr);
                }
                Ok(0)
            }
            Ok(ArchPrctlCode::GetGs) => {
                unsafe {
                    *(addr as *mut u64) = x86::msr::rdmsr(x86::msr::IA32_KERNEL_GSBASE);
                }
                Ok(0)
            }
            _ => Err(LinuxError::ENOSYS),
        }
    })
}
