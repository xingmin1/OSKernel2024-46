use core::sync::atomic::AtomicU64;

use alloc::{
    string::{String, ToString}, sync::Arc, vec::Vec
};

use arceos_posix_api::FD_TABLE;
use axerrno::{AxError, AxResult};
use axfs::{CURRENT_DIR, CURRENT_DIR_PATH};
use axhal::arch::{TrapFrame, UspaceContext};
use axmm::AddrSpace;
use axns::{AxNamespace, AxNamespaceIf};
use axsync::Mutex;
use axtask::{current, AxTaskRef, TaskExtRef, TaskInner, WeakAxTaskRef};
use bitflags::bitflags;
use heap::HeapManager;
use memory_addr::MemoryAddr;
use time::TimeStat;

mod heap;
mod time;

/// Task extended data for the monolithic kernel.
pub struct TaskExt {
    /// The process ID.
    pub proc_id: usize,
    /// The clear thread tid field
    ///
    /// See <https://manpages.debian.org/unstable/manpages-dev/set_tid_address.2.en.html#clear_child_tid>
    ///
    /// When the thread exits, the kernel clears the word at this address if it is not NULL.
    clear_child_tid: AtomicU64,
    /// The user space context.
    pub uctx: UspaceContext,
    /// The virtual memory address space.
    pub aspace: Arc<Mutex<AddrSpace>>,
    /// The heap manager
    pub heap: Arc<Mutex<HeapManager>>,
    /// The time statistics
    pub time_stat: Arc<Mutex<TimeStat>>,
    /// The resource namespace
    pub ns: AxNamespace,
    /// Parent
    pub parent: Option<WeakAxTaskRef>,
    /// Children
    pub children: Mutex<Vec<AxTaskRef>>,
}

impl TaskExt {
    pub fn new(
        proc_id: usize,
        uctx: UspaceContext,
        aspace: Arc<Mutex<AddrSpace>>,
        parent: &AxTaskRef,
    ) -> Self {
        Self {
            proc_id,
            uctx,
            clear_child_tid: AtomicU64::new(0),
            aspace,
            heap: Arc::new(Mutex::new(HeapManager::default())),
            time_stat: Arc::new(Mutex::new(TimeStat::new())),
            ns: AxNamespace::new_thread_local(),
            parent: Some(Arc::downgrade(parent)),
            children: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn clear_child_tid(&self) -> u64 {
        self.clear_child_tid
            .load(core::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn set_clear_child_tid(&self, clear_child_tid: u64) {
        self.clear_child_tid
            .store(clear_child_tid, core::sync::atomic::Ordering::Relaxed);
    }

    /// 设置父任务
    pub fn set_parent(&mut self, parent: AxTaskRef) {
        self.parent = Some(Arc::downgrade(&parent));
    }

    /// 添加子任务
    pub fn add_child(&self, child: AxTaskRef) {
        let mut children = self.children.lock();
        children.push(child);
    }

    /// 移除子任务
    pub fn remove_child(&self, child_id: usize) {
        let mut children = self.children.lock();
        if let Some(pos) = children
            .iter()
            .position(|c| c.task_ext().proc_id == child_id)
        {
            children.remove(pos);
        }
    }

    /// 获取父任务的 PID，如果父任务不存在则返回 `None`
    pub fn parent_id(&self) -> Option<usize> {
        // 由于parent引用是父进程的主进程，所以其tid就是父进程的pid。
        // 第一个进程的父进程是一个内核线程，所以这样做可以统一处理。
        self.parent
            .as_ref()
            .and_then(|parent| parent.upgrade())
            .map(|task| task.id().as_u64() as usize)
    }

    /// 进入用户态时更新时间统计
    pub fn enter_uspace(&self) {
        self.time_stat.lock().enter_uspace();
    }

    /// 进入内核态时更新时间统计
    pub fn enter_kspace(&self) {
        self.time_stat.lock().enter_kspace();
    }

    pub(crate) fn ns_init_new(&self) {
        FD_TABLE.deref_from(&self.ns).init_new(FD_TABLE.copy_inner());
        CURRENT_DIR.deref_from(&self.ns).init_new(CURRENT_DIR.copy_inner());
        CURRENT_DIR_PATH.deref_from(&self.ns).init_new(CURRENT_DIR_PATH.copy_inner());
    }
}

struct AxNamespaceImpl;

#[crate_interface::impl_interface]
impl AxNamespaceIf for AxNamespaceImpl {
    #[inline(never)]
    fn current_namespace_base() -> *mut u8 {
        let current = axtask::current();
        // Safety: We only check whether the task extended data is null and do not access it.
        if unsafe { current.task_ext_ptr() }.is_null() {
            return axns::AxNamespace::global().base();
        }
        current.task_ext().ns.base()
    }
}

axtask::def_task_ext!(TaskExt);

pub fn spawn_user_task(aspace: Arc<Mutex<AddrSpace>>, uctx: UspaceContext) -> AxTaskRef {
    let mut task = TaskInner::new(
        || {
            let curr = axtask::current();
            let kstack_top = curr.kernel_stack_top().unwrap();
            info!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                curr.task_ext().uctx.get_ip(),
                curr.task_ext().uctx.get_sp(),
                kstack_top,
            );
            unsafe { curr.task_ext().uctx.enter_uspace(kstack_top) };
        },
        "userboot".into(),
        crate::config::KERNEL_STACK_SIZE,
    );
    task.ctx_mut()
        .set_page_table_root(aspace.lock().page_table_root());
    task.init_task_ext(TaskExt::new(
        task.id().as_u64() as usize,
        uctx,
        aspace,
        current().as_task_ref(),
    ));
    task.task_ext().ns_init_new();
    axtask::spawn_task(task)
}

/// 实现简易的clone系统调用
/// 返回值为新产生的任务的id
pub fn clone_task(
    _flags: usize,
    stack: Option<usize>,
    _ptid: usize,
    _tls: usize,
    _ctid: usize,
) -> AxResult<u64> {
    let mut new_task = TaskInner::new(
        || {
            let curr = axtask::current();
            let kstack_top = curr.kernel_stack_top().unwrap();
            info!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                curr.task_ext().uctx.get_ip(),
                curr.task_ext().uctx.get_sp(),
                kstack_top,
            );
            unsafe { curr.task_ext().uctx.enter_uspace(kstack_top) };
        },
        String::from(current().id_name()),
        crate::config::KERNEL_STACK_SIZE,
    );

    let current_task = current();

    // 复制原有的地址空间
    let mut current_aspace = current_task.task_ext().aspace.lock();
    let new_aspace = current_aspace.clone_or_err()?;
    new_task
        .ctx_mut()
        .set_page_table_root(new_aspace.page_table_root());

    // 复制原有的trap上下文并设置用户空间上下文
    let trap_frame_vir_address = current_task
        .kernel_stack_top()
        .expect("no kernel stack top")
        .sub(core::mem::size_of::<TrapFrame>());
    let mut trap_frame = unsafe { *(trap_frame_vir_address.as_ptr_of::<TrapFrame>()) };
    trap_frame.sepc += 4;
    let mut new_uspace_context = UspaceContext::from(&trap_frame);
    new_uspace_context.set_retval(0);
    if let Some(stack) = stack {
        new_uspace_context.set_sp(stack);
    }

    // 初始化新任务扩展，启动新任务，维护父子关系
    let return_id = new_task.id().as_u64();
    let new_task_ext = TaskExt::new(
        return_id as usize,
        new_uspace_context,
        Arc::new(Mutex::new(new_aspace)),
        current_task.as_task_ref(),
    );
    new_task_ext.ns_init_new();
    new_task.init_task_ext(new_task_ext);
    let new_task = axtask::spawn_task(new_task);
    current_task.task_ext().add_child(new_task);
    Ok(return_id)
}

/// 等待子进程完成任务，若子进程没有完成，则自身可能会用yield轮询
/// 成功则返回进程ID；如果指定了WNOHANG，且进程还未改变状态，直接返回0；失败则返回-1；
///
/// # Safety
///
/// 保证传入的 ptr 是有效的
pub unsafe fn wait_pid(pid: i32, exit_code_ptr: *mut i32, option: i32) -> isize {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum WaitStatus {
        /// 子任务正常退出
        Exited,
        /// 子任务正在运行
        Running,
        /// 找不到对应的子任务
        NotExist,
    }
    bitflags! {
        /// 指定 sys_wait4 的选项
        #[derive(Debug, Clone, Copy)]
        pub struct WaitFlags: u32 {
            /// 不挂起当前进程，直接返回
            const WNOHANG = 1 << 0;
            /// 报告已执行结束的用户进程的状态
            const WIMTRACED = 1 << 1;
            /// 报告还未结束的用户进程的状态
            const WCONTINUED = 1 << 3;
            /// Wait for any child
            const WALL = 1 << 30;
            /// Wait for cloned process
            const WCLONE = 1 << 31;
        }
    }
    let current_task = current();

    let mut exit_task_id: usize = 0;
    let mut answer_id = 0;
    let mut answer_status;
    let options = WaitFlags::from_bits_truncate(option as u32);

    if !options.difference(WaitFlags::WNOHANG).is_empty() {
        warn!("Unsupported option: {:?}", options);
    }

    'outer: loop {
        answer_status = WaitStatus::NotExist;

        let children = current_task.task_ext().children.lock();
        for (index, child) in children.iter().enumerate() {
            if pid <= 0 {
                if pid == 0 {
                    warn!("Process group waiting is not supported.");
                }

                answer_status = WaitStatus::Running;
                let state = child.state();

                if state == axtask::TaskState::Exited {
                    let exit_code = child.exit_code();
                    answer_status = WaitStatus::Exited;

                    exit_task_id = index;
                    if !exit_code_ptr.is_null() {
                        unsafe {
                            *exit_code_ptr = exit_code << 8;
                        }
                    }
                    answer_id = child.task_ext().proc_id as usize;
                    break 'outer;
                }
            } else if child.task_ext().proc_id == pid as usize {
                if let Some(exit_code) = child.join() {
                    answer_status = WaitStatus::Exited;
                    info!(
                        "Waited for pid {} with exit code {:?}",
                        child.task_ext().proc_id,
                        exit_code
                    );

                    exit_task_id = index;
                    if !exit_code_ptr.is_null() {
                        unsafe {
                            *exit_code_ptr = exit_code << 8;
                        }
                    }
                    answer_id = child.task_ext().proc_id as usize;
                } else {
                    answer_status = WaitStatus::Running;
                }
                break 'outer;
            }
        }

        drop(children);

        if !options.contains(WaitFlags::WNOHANG) && answer_status == WaitStatus::Running {
            axtask::yield_now();
        } else {
            break;
        }
    }

    // 若进程成功结束，需要将其从父进程的children中删除
    if answer_status == WaitStatus::Exited {
        let mut children = current_task.task_ext().children.lock();
        children.remove(exit_task_id);
        answer_id as isize
    } else if options.contains(WaitFlags::WNOHANG) {
        0
    } else {
        -1
    }
}

/// 将当前进程替换为指定的用户程序
pub fn exec(program_name: &str) -> AxResult<()> {
    let current_task = current();

    // 原有的name所在页面会被unmap，所以需要提前拷贝
    let program_name = program_name.to_string();

    // 确保地址空间只被当前任务引用
    let mut aspace = current_task.task_ext().aspace.lock();
    if Arc::strong_count(&current_task.task_ext().aspace) != 1 {
        warn!("Address space is shared by multiple tasks, exec is not supported");
        return Err(AxError::Unsupported);
    }

    // 释放旧的用户地址空间
    aspace.unmap_user_areas()?;
    axhal::arch::flush_tlb(None);

    // 加载新程序，获取入口点和用户栈基地址
    let (entry_point, user_stack_base) = crate::mm::map_elf_sections(&program_name, &mut aspace)
        .map_err(|_| {
            error!("Failed to load app {}", program_name);
            AxError::NotFound
        })?;
    current_task.set_name(&program_name);

    // 更新用户上下文
    let task_ext = unsafe { &mut *(current_task.task_ext_ptr() as *mut TaskExt) };
    task_ext.uctx = UspaceContext::new(entry_point.as_usize(), user_stack_base, 0);

    // 切换到用户态
    unsafe {
        task_ext.uctx.enter_uspace(
            current_task
                .kernel_stack_top()
                .expect("No kernel stack top"),
        );
    }
}
