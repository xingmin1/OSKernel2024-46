use core::sync::atomic::AtomicU64;

use alloc::{string::String, sync::Arc, vec::Vec};

use axerrno::AxResult;
use axhal::arch::{TrapFrame, UspaceContext};
use axmm::AddrSpace;
use axns::{AxNamespace, AxNamespaceIf};
use axsync::Mutex;
use axtask::{current, AxTaskRef, TaskExtRef, TaskInner, WeakAxTaskRef};
use memory_addr::MemoryAddr;

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
    /// The resource namespace
    pub ns: AxNamespace,
    /// Parent
    pub parent: Option<WeakAxTaskRef>,
    /// Children
    pub children: Mutex<Vec<AxTaskRef>>,
}

impl TaskExt {
    pub fn new(proc_id: usize, uctx: UspaceContext, aspace: Arc<Mutex<AddrSpace>>, parent: &AxTaskRef) -> Self {
        Self {
            proc_id,
            uctx,
            clear_child_tid: AtomicU64::new(0),
            aspace,
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
        if let Some(pos) = children.iter().position(|c| c.task_ext().proc_id == child_id) {
            children.remove(pos);
        }
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
    task.init_task_ext(TaskExt::new(task.id().as_u64() as usize, uctx, aspace, current().as_task_ref()));
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
    new_task.ctx_mut().set_page_table_root(new_aspace.page_table_root());

    // 复制原有的trap上下文并设置用户空间上下文
    let trap_frame_vir_address = current_task.kernel_stack_top().expect("no kernel stack top").sub(core::mem::size_of::<TrapFrame>());
    let mut trap_frame = unsafe { *(trap_frame_vir_address.as_ptr_of::<TrapFrame>()) };
    trap_frame.sepc += 4;
    let mut new_uspace_context = UspaceContext::from(&trap_frame);
    new_uspace_context.set_retval(0);
    if let Some(stack) = stack {
        new_uspace_context.set_sp(stack);
    }
    
    // 初始化新任务扩展，启动新任务，维护父子关系
    let return_id = new_task.id().as_u64();
    new_task.init_task_ext(TaskExt::new(
        return_id as usize,
        new_uspace_context,
        Arc::new(Mutex::new(new_aspace)),
        current_task.as_task_ref(),
    ));
    let new_task = axtask::spawn_task(new_task);
    current_task.task_ext().add_child(new_task);
    Ok(return_id)
}