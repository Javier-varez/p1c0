//! This module implements an early allocation policy for the MMU. This allocator should not be
//! used by any other piece of code and will explicitly be used only before initialization of the
//! MMU is done. Afterwards, usage of the global heap allocator is recommended.
//!
//! This allocator will not deal with limitations such as reentrancy or deallocation (will leak all
//! memory) for the purposes of simplicity. Such constraints shall be fine as long as the following
//! conditions are upheld by all usages:
//!
//! SAFETY:
//!   * All uses of the allocator will happen in the same thread by the initialization code of the
//!     MMU.
//!   * After the MMU is initialized, this allocator will not be called anymore. This will be
//!     checked during runtime to make sure no such operation happens.
//!

use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    cell::RefCell,
    ptr::NonNull,
};

// Align this with the page size, since almost all early allocations will need to be aligned to
// the 16kB page size (corresponding to table pointers).
#[derive(Debug)]
#[repr(C, align(0x4000))]
pub(super) struct EarlyAllocator<const SIZE: usize> {
    memory: [u8; SIZE],
    offset: RefCell<usize>,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            memory: [0; SIZE],
            offset: RefCell::new(0),
        }
    }
}

/// SAFETY:
///   This allocator is not meant to be used in a multithreaded context. By contract, it is only
///   used in a single-threaded context and only during the initial MMU startup. It shall still be
///   Sync as it will be allocated statically and we cannot enforce by compiler rules that it will
///   be accessed by just one thread.
unsafe impl<const SIZE: usize> Sync for EarlyAllocator<SIZE> {}

/// SAFETY:
///   The early allocator is safe because it will always return memory that points to a pool of
///   memory inside itself. The memory given by this allocator is guaranteed to be
unsafe impl<const SIZE: usize> GlobalAlloc for EarlyAllocator<SIZE> {
    /// SAFETY:
    ///   Caller must ensure that layout has non-zero size. Otherwise this call results in
    ///   undefined behavior.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut offset_borrow = self.offset.borrow_mut();
        let offset = align_up(self.memory.as_ptr(), *offset_borrow, layout.align());
        if (layout.size() + offset) > SIZE {
            return core::ptr::null_mut();
        }

        let ptr = self.memory.as_ptr().add(offset) as *mut _;

        // Move the offset to account for the layout size and alignment
        *offset_borrow = offset + layout.size();

        // SAFETY: The pointer is guaranteed to not be null, as it is an offset into the memory
        // array
        ptr
    }

    /// SAFETY:
    ///   Caller must ensure that `ptr` points to a memory block that was obtained with this
    ///   allocator instance and using the same layout passed to this dealloc call.
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // For now let's just leak the memory...
    }
}

fn align_up(base: *const u8, offset: usize, alignment: usize) -> usize {
    // To calculate the alignment we need to take into account the offset
    let ptr = base as usize + offset;

    let remainder = ptr % alignment;
    if remainder == 0 {
        ptr - base as usize
    } else {
        ptr + alignment - remainder - base as usize
    }
}

#[derive(Debug, Clone)]
pub(super) struct AllocRef<'a, T: GlobalAlloc>(&'a T);

impl<'a, T: GlobalAlloc> AllocRef<'a, T> {
    pub fn new(reference: &'a T) -> Self {
        Self(reference)
    }
}

/// SAFETY:
///   The underlying memory is valid at least until all allocators that reference the same
///   underlying allocator are dropped.
///   This allocator also does not own the memory that is given out, and since the underlying is
///   behind a shared reference, cloning this AllocRef does not invalidate the memory blocks
///   already given by either the underlying allocator or the AllocRef being cloned.
unsafe impl<'a, T: GlobalAlloc> Allocator for AllocRef<'a, T> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = unsafe { self.0.alloc(layout) };

        if ptr.is_null() {
            return Err(AllocError);
        }

        unsafe {
            Ok(NonNull::new_unchecked(core::slice::from_raw_parts_mut(
                ptr,
                layout.size(),
            )))
        }
    }

    /// SAFETY:
    ///   `ptr` must point to a memory block allocated by this allocator and still valid.
    ///   `layout` must correspond to the same Layout used in the original allocate call from which
    ///   `ptr` was obtained.
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.0.dealloc(ptr.as_ptr(), layout)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct EarlyAllocatorTest {
        allocator: EarlyAllocator<1024>,
    }

    impl EarlyAllocatorTest {
        fn new() -> Self {
            Self {
                allocator: EarlyAllocator::new(),
            }
        }

        fn get_base(&self) -> *mut u8 {
            &self.allocator.memory[0] as *const _ as *mut _
        }

        fn get_offset(&self) -> usize {
            *self.allocator.offset.borrow()
        }
    }

    #[test]
    fn allocator() {
        let test = EarlyAllocatorTest::new();

        let ptr = unsafe { test.allocator.alloc(Layout::new::<u128>()) };

        assert_eq!(ptr, test.get_base());
        assert_eq!(test.get_offset(), 16);

        let ptr = unsafe { test.allocator.alloc(Layout::new::<u8>()) };

        assert_eq!(ptr, unsafe { test.get_base().add(16) });
        assert_eq!(test.get_offset(), 17);

        let ptr = unsafe { test.allocator.alloc(Layout::new::<u32>()) };

        assert_eq!(ptr, unsafe { test.get_base().add(20) });
        assert_eq!(test.get_offset(), 24);
    }

    #[test]
    fn allocator_ref() {
        let test = EarlyAllocatorTest::new();

        let mut vector = Vec::new_in(AllocRef::new(&test.allocator));

        assert_eq!(test.get_offset(), 0);

        vector.push(0u32);

        assert_eq!(&vector[0] as *const _ as *mut _, unsafe {
            test.get_base().add(0)
        });
        assert!(test.get_offset() > std::mem::size_of::<u32>());

        vector.push(1u32);

        assert_eq!(&vector[1] as *const _ as *mut _, unsafe {
            test.get_base().add(std::mem::size_of::<u32>())
        });
        assert!(test.get_offset() > std::mem::size_of::<u32>() * 2);
    }
}
