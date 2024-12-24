use axtask::{current, TaskExtRef};

pub struct TimeStat {
    /// 在用户态流过的累计时间
    user_time: u64,
    /// 在内核态流过的累计时间
    kernel_time: u64,
    /// 最近一次进入用户态的时间
    last_user_time: u64,
    /// 最近一次进入内核态的时间
    last_kernel_time: u64,
}

impl TimeStat {
    pub fn new() -> Self {
        debug!(
            "TimeStat::new. Current ticks: {}",
            axhal::time::current_ticks()
        );
        TimeStat {
            user_time: 0,
            kernel_time: 0,
            last_user_time: 0,
            last_kernel_time: axhal::time::current_ticks(),
        }
    }

    pub fn enter_uspace(&mut self) {
        debug!(
            "TimeStat::enter_uspace. Current ticks: {}",
            axhal::time::current_ticks()
        );
        let current_time = axhal::time::current_ticks();
        self.last_user_time = current_time;
        self.kernel_time += current_time - self.last_kernel_time;
    }

    pub fn enter_kspace(&mut self) {
        debug!(
            "TimeStat::enter_kspace. Current ticks: {}",
            axhal::time::current_ticks()
        );
        let current_time = axhal::time::current_ticks();
        self.last_kernel_time = current_time;
        self.user_time += current_time - self.last_user_time;
    }

    pub fn info(&self) -> (u64, u64) {
        (self.user_time, self.kernel_time)
    }
}

impl Default for TimeStat {
    fn default() -> Self {
        Self::new()
    }
}

#[axhal::trap::register_trap_handler(axhal::trap::BEFORE_ALL_TRAPS)]
fn before_all_traps() {
    let current_task = current();

    // 避开只有内核线程的情况,如 idle 线程等
    if !unsafe { current_task.task_ext_ptr() }.is_null() {
        current_task.task_ext().time_stat.lock().enter_uspace();
    }
}

#[axhal::trap::register_trap_handler(axhal::trap::AFTER_ALL_TRAPS)]
fn after_all_traps() {
    let current_task = current();

    // 避开只有内核线程的情况,如 idle 线程等
    if !unsafe { current_task.task_ext_ptr() }.is_null() {
        current_task.task_ext().time_stat.lock().enter_kspace();
    }
}
