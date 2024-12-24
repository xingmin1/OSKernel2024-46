//! Trap handling.

use linkme::distributed_slice as def_trap_handler;
use memory_addr::VirtAddr;
use page_table_entry::MappingFlags;

#[cfg(feature = "uspace")]
use crate::arch::TrapFrame;

pub use linkme::distributed_slice as register_trap_handler;

/// A slice of IRQ handler functions.
#[def_trap_handler]
pub static IRQ: [fn(usize) -> bool];

/// A slice of page fault handler functions.
#[def_trap_handler]
pub static PAGE_FAULT: [fn(VirtAddr, MappingFlags, bool) -> bool];

/// A slice of syscall handler functions.
#[cfg(feature = "uspace")]
#[def_trap_handler]
pub static SYSCALL: [fn(&TrapFrame, usize) -> isize];

// 先将 uspace feature 当做 monolithic feature 使用
#[cfg(feature = "uspace")]
#[def_trap_handler]
pub static BEFORE_ALL_TRAPS: [fn()];

#[cfg(feature = "uspace")]
#[def_trap_handler]
pub static AFTER_ALL_TRAPS: [fn()];

#[allow(unused_macros)]
macro_rules! handle_trap {
    ($trap:ident, $($args:tt)*) => {{
        // 目前主要用于统计时间
        #[cfg(feature = "uspace")]
        if let Some(func) = $crate::trap::BEFORE_ALL_TRAPS.iter().next() {
            func();
        }

        let mut iter = $crate::trap::$trap.iter();
        let ret = if let Some(func) = iter.next() {
            if iter.next().is_some() {
                warn!("Multiple handlers for trap {} are not currently supported", stringify!($trap));
            }
            func($($args)*)
        } else {
            warn!("No registered handler for trap {}", stringify!($trap));
            false
        };

        // 目前主要用于统计时间
        #[cfg(feature = "uspace")]
        if let Some(func) = $crate::trap::AFTER_ALL_TRAPS.iter().next() {
            func();
        }

        ret
    }}
}

/// Call the external syscall handler.
#[cfg(feature = "uspace")]
pub(crate) fn handle_syscall(tf: &TrapFrame, syscall_num: usize) -> isize {
    SYSCALL[0](tf, syscall_num)
}
