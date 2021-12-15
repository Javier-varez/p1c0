extern crate alloc;

use alloc::alloc::Global;
use alloc::vec::Vec;

use core::alloc::{AllocError, Allocator, Layout};

use core::ptr::NonNull;

#[repr(transparent)]
pub struct AlignedAllocator<A: Allocator, const ALIGNMENT: usize> {
    inner: A,
}

impl<const ALIGNMENT: usize> AlignedAllocator<Global, ALIGNMENT> {
    pub fn new() -> Self {
        Self { inner: Global }
    }
}

unsafe impl<A: Allocator, const ALIGNMENT: usize> Allocator for AlignedAllocator<A, ALIGNMENT> {
    fn allocate(&self, mut layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.align() < ALIGNMENT {
            layout = layout.align_to(ALIGNMENT).expect("Can be aligned");
        }
        self.inner.allocate(layout)
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, mut layout: Layout) {
        if layout.align() < ALIGNMENT {
            layout = layout.align_to(ALIGNMENT).expect("Can be aligned");
        }
        self.inner.deallocate(ptr, layout);
    }
}

pub type AlignedVec<T, const ALIGNMENT: usize> = Vec<T, AlignedAllocator<Global, ALIGNMENT>>;

pub fn new_aligned_vector<T, const ALIGNMENT: usize>() -> AlignedVec<T, ALIGNMENT> {
    AlignedVec::new_in(AlignedAllocator::new())
}
