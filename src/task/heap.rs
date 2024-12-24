use axhal::paging::MappingFlags;
use axtask::{current, TaskExtRef};
use memory_addr::VirtAddr;

#[derive(Debug, Clone, Copy)]
pub struct HeapManager {
    heap_top: VirtAddr,
    actual_heap_top: VirtAddr,
}

impl HeapManager {
    pub fn empty() -> Self {
        Self {
            heap_top: VirtAddr::from_usize(crate::config::USER_HEAP_BOTTOM),
            actual_heap_top: VirtAddr::from_usize(crate::config::USER_HEAP_BOTTOM),
        }
    }

    /// 成功时返回新的实际堆顶，失败时返回None
    /// top: 新的实际堆顶
    /// 当top == 0时，返回当前实际堆顶
    /// 当top高(低)于堆的范围时，返回None
    /// 当map_(de)alloc失败时，返回None
    pub fn set_heap_top(&mut self, top: VirtAddr) -> Option<VirtAddr> {
        debug!("Set heap top: {:#x?}", top);
        if top.as_usize() == 0 {
            Some(self.heap_top)
        } else if top > self.heap_top {
            self.alloc(top)
        } else {
            self.dealloc(top)
        }
    }

    /// 成功时返回新的实际堆顶，失败时返回None
    /// top: 新的实际堆顶
    /// 当top高于堆的范围时，返回None
    /// 当map_alloc失败时，返回None
    fn alloc(&mut self, top: VirtAddr) -> Option<VirtAddr> {
        debug!("Alloc heap top: {:#x?}", top);
        if top.as_usize() > crate::config::USER_HEAP_BOTTOM + crate::config::USER_HEAP_SIZE {
            debug!("Heap top out of range: {:#x?}", top);
            return None;
        }

        if top <= self.actual_heap_top {
            self.heap_top = top;
            return Some(top);
        }

        let aligned_top: VirtAddr = memory_addr::align_up_4k(top.as_usize()).into();
        current()
            .task_ext()
            .aspace
            .lock()
            .map_alloc(
                self.actual_heap_top,
                aligned_top - self.actual_heap_top,
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
                false,
            )
            .ok()?;

        self.heap_top = top;
        self.actual_heap_top = aligned_top;
        Some(top)
    }

    /// 成功时返回新的实际堆顶，失败时返回None
    /// top: 新的实际堆顶
    /// 当top低于堆的范围时，返回None
    /// 当map_dealloc失败时，返回None
    fn dealloc(&mut self, top: VirtAddr) -> Option<VirtAddr> {
        debug!("Dealloc heap top: {:#x?}", top);
        if top.as_usize() < crate::config::USER_HEAP_BOTTOM {
            debug!("Heap top out of range: {:#x?}", top);
            return None;
        }

        self.heap_top = top;
        let aligned_top: VirtAddr = memory_addr::align_up_4k(top.as_usize()).into();
        if aligned_top < self.actual_heap_top {
            current()
                .task_ext()
                .aspace
                .lock()
                .unmap(aligned_top, self.actual_heap_top - aligned_top)
                .ok()?;
            self.actual_heap_top = aligned_top;
        }
        Some(top)
    }
}

impl Default for HeapManager {
    fn default() -> Self {
        Self::empty()
    }
}
