use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

#[cfg(not(test))]
#[global_allocator]
static ALLOCATOR: TerribleAllocator = TerribleAllocator::new();

#[cfg(test)]
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

#[cfg(test)]
mod test {
    use super::*;

    struct Test {
        arena: Vec<u32>,
    }

    impl Test {
        fn new() -> Self {
            let mut test = Self {
                arena: vec![0xFFAA5500u32; 1024],
            };
            let base = &mut test.arena[0] as *mut _ as *mut _;
            unsafe { ALLOCATOR.init(base, test.size()) };
            test
        }

        fn base(&self) -> *const u8 {
            &self.arena[0] as *const _ as *const _
        }

        fn size(&self) -> usize {
            core::mem::size_of_val(&self.arena)
        }
    }

    #[test]
    fn allocate_i32() {
        let test = Test::new();

        let layout = Layout::for_value(&30u32);
        let ptr = unsafe { ALLOCATOR.alloc(layout) };

        assert_eq!(ptr as *const _, test.base());

        let layout = Layout::for_value(&30u32);
        let ptr = unsafe { ALLOCATOR.alloc(layout) };

        assert_eq!(ptr as *const _, unsafe {
            test.base().add(core::mem::size_of::<u32>())
        });
    }

    #[test]
    fn allocate_i64() {
        let test = Test::new();

        let layout = Layout::for_value(&30u32);
        let ptr = unsafe { ALLOCATOR.alloc(layout) };

        assert_eq!(ptr as *const _, test.base());

        let layout = Layout::for_value(&30u64);
        let ptr = unsafe { ALLOCATOR.alloc(layout) };

        assert_eq!(ptr as *const _, unsafe {
            test.base().add(core::mem::size_of::<u64>())
        });
    }

    #[test]
    fn deallocate_doesnt_actually_free() {
        let test = Test::new();

        let layout = Layout::for_value(&30u32);
        let ptr = unsafe { ALLOCATOR.alloc(layout) };
        assert_eq!(ptr as *const _, test.base());

        unsafe { ALLOCATOR.dealloc(ptr, layout) };

        let layout = Layout::for_value(&30u64);
        let ptr = unsafe { ALLOCATOR.alloc(layout) };

        assert_eq!(ptr as *const _, unsafe {
            test.base().add(core::mem::size_of::<u64>())
        });
    }
}
