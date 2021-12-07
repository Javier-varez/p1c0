use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

#[global_allocator]
static ALLOCATOR: TerribleAllocator = TerribleAllocator::new();

unsafe impl Sync for TerribleAllocator {}

struct TerribleAllocator {
    base_addr: UnsafeCell<*mut u8>,
    total_size: UnsafeCell<usize>,
    allocated_size: UnsafeCell<usize>,
}

impl TerribleAllocator {
    const fn new() -> Self {
        Self {
            base_addr: UnsafeCell::new(core::ptr::null_mut()),
            total_size: UnsafeCell::new(0),
            allocated_size: UnsafeCell::new(0),
        }
    }

    unsafe fn init(&self, base_addr: *mut u8, size: usize) {
        *self.base_addr.get() = base_addr;
        *self.total_size.get() = size;
        *self.allocated_size.get() = 0;
    }
}

fn align_up(value: usize, alignment: usize) -> usize {
    let remainder = value % alignment;
    if remainder == 0 {
        value
    } else {
        value + alignment - remainder
    }
}

pub unsafe fn init(base_addr: *mut u8, size: usize) {
    ALLOCATOR.init(base_addr, size);
}

/// SAFETY:
/// Memory can potentially be allocated anywhere in the program. We would need to have a
/// locking mechanism or at least use atomics. Since we have none of that at the moment, we
/// are just going to ignore all safety! (not really a problem since the program is
/// essentially single-threaded at the moment).
unsafe impl GlobalAlloc for TerribleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let allocated_size = align_up(*self.allocated_size.get(), layout.align());
        if layout.size() > (*self.total_size.get() - allocated_size) {
            return core::ptr::null_mut();
        }

        *self.allocated_size.get() = allocated_size + layout.size();
        (*self.base_addr.get()).add(allocated_size)
    }

    /// We just don't free any memory! Leaking is safe after all, isn't it? =D
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
