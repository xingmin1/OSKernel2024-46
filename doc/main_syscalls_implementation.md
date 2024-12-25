# 主要系统调用的实现

## 1. `sys_mount`

### 文件系统层面的支持

原系统提供了一个挂载的函数

```rust
axfs::root::RootDirectory 
pub fn mount(&self, path: &'static str, fs: Arc<dyn VfsOps>) -> AxResult
```

，可以挂载一个vfs。而且我觉得这样实现是比较合理的，于是我就接着实现下去。

原系统为外部库`fatfs::FileSystem`的Wrapper（`FatFileSystem`）及其节点的Wrapper（`FileWrapper``DirWrapper`）分别实现了`VfsOps`和`VfsNode`。然后由于打开的镜像文件文件的存在形式为`fatfs::File`，于是我就以`pub struct FileWrapper<'a, IO: IoTrait>(Mutex<File<'a, IO, NullTimeProvider, LossyOemCpConverter>>)`为基础，新建了一个文件系统类型

```rust
pub struct FatFileSystemFromFile {
    inner: fatfs::FileSystem<FileWrapper<'static, Disk>, NullTimeProvider, LossyOemCpConverter>,
    root_dir: UnsafeCell<Option<VfsNodeRef>>,
}
```

，然后修改了`VfsOps`的实现，和`FileWrapper``DirWrapper`的定义，使它们为泛型，使的`FatFileSystemFromFile`能复用`FatFileSystem`的大部分实现。

### 系统调用的实现

先处理路径，然后判断传入的文件系统类型是不是`vfat`（目前仅支持`vfat`），然后根据设备路径打开文件，再用文件创建一个新的`vfs`，然后保存到一个数组中。在之后根据路径打开文件时，会匹配所有的`vfs`，根据最长的匹配路径，找到对应的`vfs`，然后再调用`vfs`的根目录的`open`函数。

对于`mount`测例，由于`/dev/vda2`本身不存在，而在原系统的实现中，在`/dev`挂载了一个其他的文件系统，如果要创建一个在`/dev`下的文件，需要修改那个文件系统的实现，而那个文件系统的实现以外部库的形式存在。碰巧，在`mount`测例中支持传参改变设备路径，于是就先创建一个`vfat12`的系统镜像，然后嵌入kernel中，在执行测例前创建一个文件，并将那个镜像文件拷贝过去，然后再将文件路径传给测例，这样就有了挂载的基础了。

## 2. `sys_brk`

### 实现思路

1. 在配置文件中预先选定一块与世无争的内存区域，然后定义堆的起始地址和大小。

2. 对于heap的管理，由于分配时是按页分配，所以接下来需要保存堆顶和实际的堆顶，然后当堆顶变化时，且变化后会跨域对其边界时，就将更新实际的堆顶为堆顶的向上对其的值。此外，还要时刻满足堆底 <= 堆顶 <= 实际堆顶 <= 堆底 + 堆大小 的要求。另外，分配的内存是懒加载的。

### 测例

在`brk`测例中，`brk`系统调用的返回值类型为`int`，所以堆的地址不能太大，不然会要到问题，比如变成负数，导致出现问题。

## 3. `sys_times`

### 实现思路

1. 创建一个结构体，用于保存时间信息，然后在进入内核空间、进入用户空间时更新信息。

```rust
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
```

2. 从进程存在空间的角度来说，进程有三种状态：不于在任何空间(a)、在内核空间(b)、在用户空间(c)。且一般都是从(a) -> (b) -> (c) -> (b) -> (c) -> (b) -> (a)这样的状态转移，a位于两端，中间从b开始，然后c、b交替，最后以b结束。可以看到没有连续的b、c，所以每次进入一个空间是，记一下时间，退出时在此空间累计时间上加上时间差，然后再记一下时间。这样看来，我当时的结构体的实现是有一个多余字段的，即`last_user_time`和`last_kernel_time`中有一个多余，因为这两个字段的值是在进入用户空间和内核空间时以当前时间覆盖，而在退出时只是读一下，其实读之后就可以再次使用了。

## `sys_fstat`

### 实现

在实现`sys_fstat`时，没有没有注意到`stat`和`kstat`的区别，导致在测例中，显示的结果比较奇怪。后来发现了这个问题，就将`stat`的转化为`kstat`再返回。

## `sys_linkat`

### 实现

1. 为实现链接，建立了一个硬链接管理器，在比较浅的层面实现链接的管理。

```rust
pub struct HardlinkManager {
    inner: RwLock<LinkManagerInner>,
}
struct LinkManagerInner {
    links: BTreeMap<String, String>,
    ref_counts: BTreeMap<String, usize>,
}
```

2. 创建新链接时，就在`links`中添加一个新的映射，然后在`ref_counts`中递增对应的值。删除链接时，删去`links`中的映射，然后在`ref_counts`中递减对应的值，如果递减后为0，就删除被取消链接的这个文件，若在`link`中没有对应的链接，则直接删除文件。但这里有一个问题，若被链接的文件要删除，会直接被删除，其他链接到这个文件的文件也会作废。

## 关于进程模型

下面是Unikernel中的task结构

```rust
pub struct TaskInner {
    id: TaskId,
    name: UnsafeCell<String>,
    is_idle: bool,
    is_init: bool,

    entry: Option<*mut dyn FnOnce()>,
    state: AtomicU8,

    in_wait_queue: AtomicBool,
    #[cfg(feature = "irq")]
    in_timer_list: AtomicBool,

    #[cfg(feature = "preempt")]
    need_resched: AtomicBool,
    #[cfg(feature = "preempt")]
    preempt_disable_count: AtomicUsize,

    exit_code: AtomicI32,
    wait_for_exit: WaitQueue,

    kstack: Option<TaskStack>,
    ctx: UnsafeCell<TaskContext>,
    task_ext: AxTaskExt,

    #[cfg(feature = "tls")]
    tls: TlsArea,
}

/// A wrapper of pointer to the task extended data.
pub(crate) struct AxTaskExt {
    ptr: *mut u8,
}
```

在，宏内核下对其进行了扩展

```rust
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
```

其中，最需要说明的是`ns`字段，它的主要实现是在原系统上已经实现，不过在开启`thread-local`feature后有一个（我认为的）bug，然后进行了修复尝试，具体见[f0f4a3c2](https://gitlab.eduxiji.net/T202410459994713/oskernel2024-46/-/commit/f0f4a3c24924bea20e40d7532499f4eb793bba3e)。

`ns`的主要作用是在Unikernel或宏内核中对当前任务的资源获取的方式相同，以让Unikernel 已实现的系统服务最⼤程度地被宏内核复⽤，且在此基础上可以让资源定义分散在各组件中，以及任务间细粒度资源共享。

`ns`的主要原理是，它所管理的资源的布局由 `link_section` 组织，在编译时就已经确定，然后在Unikernel中全局⼀份，静态分配；在宏内核中，维护在上面的`TaskExt`中，每任务⼀份，动态分配。这样在Unikernel或宏内核中对当前任务的资源获取的方式就相同了。而我做的修改主要就是（在我的理解下），使在宏内核中，内核线程（非宏内核进程的线程）使用的`ns`不会影响到用户进程的`ns`。
