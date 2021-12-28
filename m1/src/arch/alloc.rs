use core::{
    alloc::{GlobalAlloc, Layout},
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use spin::Mutex;

#[cfg(not(test))]
#[global_allocator]
static ALLOCATOR: LockedHeapAllocator = LockedHeapAllocator::new();

#[cfg(test)]
static ALLOCATOR: LockedHeapAllocator = LockedHeapAllocator::new();

pub unsafe fn init(base_addr: *mut u8, size: usize) {
    ALLOCATOR.lock().init(base_addr, size);
}

fn aligned_address_with_layout(
    layout: Layout,
    address: *mut u8,
    mut size: usize,
) -> Option<(*mut u8, usize)> {
    let mut address = address as usize;
    let alignment = layout.align();
    let alignment_mask = alignment - 1;
    let correction = if (address & alignment_mask) != 0 {
        alignment - (address % alignment)
    } else {
        0
    };

    if layout.size() + correction > size {
        // We don't have enough memory to fit the object with the alignment correction
        return None;
    }

    address += correction;
    size -= correction + layout.size();

    Some((address as *mut u8, size))
}

struct ListEntry {
    size: usize,
    next: *mut ListEntry,
}

impl ListEntry {
    unsafe fn allocate_at_address(address: *mut u8, size: usize) -> *mut ListEntry {
        let layout = Layout::new::<Self>().pad_to_align();

        let alignment_result = aligned_address_with_layout(Layout::new::<Self>(), address, size);
        if alignment_result.is_none() {
            return core::ptr::null_mut();
        }

        let (address, mut size) = alignment_result.unwrap();
        size += layout.size();

        let head = ListEntry {
            size,
            next: core::ptr::null_mut(),
        };

        let head_ref = &mut *(address as *mut MaybeUninit<ListEntry>);
        head_ref.write(head);
        head_ref.assume_init_mut() as *mut ListEntry
    }

    unsafe fn remove_entry(entry: &mut *mut ListEntry) {
        let next = (**entry).next;
        *entry = next;
    }

    unsafe fn append_before(head: &mut *mut ListEntry, entry: *mut ListEntry) {
        let current = *head;
        (*entry).next = current;
        (*head) = entry;
    }
}

struct HeapAllocator {
    head: *mut ListEntry,
}

impl HeapAllocator {
    const fn new() -> Self {
        Self {
            head: core::ptr::null_mut(),
        }
    }

    unsafe fn init(&mut self, base_addr: *mut u8, size: usize) {
        self.head = ListEntry::allocate_at_address(base_addr, size);
    }

    fn adapt_layout(layout: Layout) -> Layout {
        let list_entry_layout: Layout = Layout::new::<ListEntry>();
        if layout.align() < list_entry_layout.size() {
            layout
                .align_to(list_entry_layout.size())
                .unwrap()
                .pad_to_align()
        } else {
            layout.pad_to_align()
        }
    }

    unsafe fn alloc(&mut self, mut layout: Layout) -> *mut u8 {
        // We force alignment to at least the same of the ListEntry
        layout = Self::adapt_layout(layout);

        // Walk the free list to split/remove an entry matching the requested allocation
        let mut entry = &mut self.head;
        while *entry != core::ptr::null_mut() {
            let entry_base = *entry as *mut u8;
            let entry_size = (**entry).size;
            if let Some((ptr, remaining_entry_size)) =
                aligned_address_with_layout(layout, entry_base, entry_size)
            {
                // Remove the entry, since now it will be allocated
                ListEntry::remove_entry(entry);

                let space_after = remaining_entry_size;
                if space_after > 0 {
                    let base_after = ptr.add(layout.size());
                    let entry_after_allocation =
                        ListEntry::allocate_at_address(base_after, space_after);
                    ListEntry::append_before(entry, entry_after_allocation);
                }

                let space_before = ptr.offset_from(entry_base) as usize;
                if space_before > 0 {
                    let entry_before_allocation =
                        ListEntry::allocate_at_address(entry_base, space_before);
                    ListEntry::append_before(entry, entry_before_allocation);
                }

                // Finally return the allocated size
                return ptr;
            } else {
                // It doesn't fit :( continue searching
                entry = &mut (**entry).next;
            }
        }
        core::ptr::null_mut()
    }

    unsafe fn can_be_consolidated(prev: *mut ListEntry, next: *mut ListEntry) -> bool {
        // If adding the size of the previous entry reaches the next entry, they could be
        // consolidated into a single entry
        (prev as *mut u8).add((*prev).size) == next as *mut u8
    }

    unsafe fn append_and_consolidate_entries(
        prev_dbl_ptr: &mut *mut ListEntry,
        new_entry: *mut ListEntry,
    ) {
        if Self::can_be_consolidated(*prev_dbl_ptr, new_entry) {
            // Simply increment the size of the current entry, no need to append another entry
            (**prev_dbl_ptr).size += (*new_entry).size;

            // Now check if the next one could also be consolidated
            let next_entry_dbl_ptr = &mut (**prev_dbl_ptr).next;
            if Self::can_be_consolidated(*prev_dbl_ptr, *next_entry_dbl_ptr) {
                // Update the size of the first entry
                (**prev_dbl_ptr).size += (**next_entry_dbl_ptr).size;

                // And remove the next entry
                ListEntry::remove_entry(next_entry_dbl_ptr);
            }
        } else {
            // Append the entry, then check if the next one could be consolidated
            let next_entry_dbl_ptr = &mut (**prev_dbl_ptr).next;
            ListEntry::append_before(next_entry_dbl_ptr, new_entry);

            if Self::can_be_consolidated(new_entry, *next_entry_dbl_ptr) {
                // Update the size of the first entry
                (*new_entry).size += (**next_entry_dbl_ptr).size;

                // And remove the next entry
                ListEntry::remove_entry(next_entry_dbl_ptr);
            }
        }
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, mut layout: Layout) {
        // We force alignment to at least the same of the ListEntry
        layout = Self::adapt_layout(layout);

        let new_entry = ListEntry::allocate_at_address(ptr, layout.size());
        // Find the entry where we will append the new block
        //
        if ptr.offset_from(self.head as *mut u8) < 0 {
            // If it is before the current entry, then we add it right away
            let old_entry = self.head;
            self.head = new_entry;
            Self::append_and_consolidate_entries(&mut self.head, old_entry);
            return;
        }

        let mut entry = &mut self.head;
        while *entry != core::ptr::null_mut() {
            let next = (**entry).next;
            if next == core::ptr::null_mut() || ptr.offset_from(next as *mut u8) < 0 {
                // We need to insert it here! The next one might already be too late
                Self::append_and_consolidate_entries(entry, new_entry);
                return;
            }

            // Next entry
            entry = &mut (**entry).next;
        }

        // Hmm, we reached the end and still didn't add it, this seems like it could only happen
        // once all memory is exhausted.
        self.head = new_entry;
    }
}

#[repr(transparent)]
struct LockedHeapAllocator(Mutex<HeapAllocator>);

impl LockedHeapAllocator {
    const fn new() -> Self {
        Self(Mutex::new(HeapAllocator::new()))
    }
}

impl Deref for LockedHeapAllocator {
    type Target = Mutex<HeapAllocator>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for LockedHeapAllocator {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

unsafe impl GlobalAlloc for LockedHeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock().alloc(layout)
    }

    /// We just don't free any memory! Leaking is safe after all, isn't it? =D
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock().dealloc(ptr, layout)
    }
}

/// Safety:
/// The allocator is behind a lock, so it is not possible to mutate it without holding the lock.
/// Internal pointers/references don't leak to the outside of this type, which means they are only
/// used internally
unsafe impl Sync for LockedHeapAllocator {}

/// Safety:
/// The allocator is behind a lock, so it is not possible to mutate it without holding the lock.
/// Internal pointers/references don't leak to the outside of this type, which means they are only
/// used internally
unsafe impl Send for LockedHeapAllocator {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn aligned_address_test() {
        let layout = Layout::new::<u32>();
        assert_eq!(
            aligned_address_with_layout(layout, 0xFFFF5504 as *mut u8, 10),
            Some((0xFFFF5504 as *mut u8, 6))
        );

        assert_eq!(
            aligned_address_with_layout(layout, 0xFFFF5501 as *mut u8, 10),
            Some((0xFFFF5504 as *mut u8, 3))
        );

        let layout = Layout::new::<usize>();
        assert_eq!(
            aligned_address_with_layout(layout, 0xFFFF5501 as *mut u8, 10),
            None
        );
    }

    struct ListEntryDesc {
        offset: usize,
        size: usize,
    }

    impl ListEntryDesc {
        const fn new(offset: usize, size: usize) -> Self {
            Self { offset, size }
        }
    }

    struct HeapTest {
        arena: Vec<u32>,
        allocator: HeapAllocator,
    }

    impl HeapTest {
        fn new() -> Self {
            let mut arena = vec![0xFFAA5500u32; 1024];
            let mut allocator = HeapAllocator::new();
            unsafe {
                allocator.init(
                    arena.as_mut_ptr() as *mut _,
                    arena.len() * std::mem::size_of::<u32>(),
                );
            }
            Self { arena, allocator }
        }

        fn base(&self) -> *const u8 {
            let ptr = self.arena.as_ptr() as *const u8;
            let offset = ptr.align_offset(16);
            unsafe { ptr.add(offset) }
        }

        fn size(&self) -> usize {
            let ptr = self.arena.as_ptr() as *const u8;
            let offset = ptr.align_offset(16);
            self.arena.len() * std::mem::size_of::<u32>() - offset
        }

        fn validate_free_list(&self, expected_entries: &[ListEntryDesc]) {
            let mut entry = self.allocator.head;

            for expected_entry in expected_entries.iter() {
                let expected_ptr = unsafe {
                    self.base().offset(expected_entry.offset as isize) as *mut u8 as *mut _
                };
                assert_eq!(expected_ptr, entry);
                assert_eq!(expected_entry.size, unsafe { (*entry).size });

                unsafe {
                    entry = (*entry).next;
                }
            }
            // This is the end of the list
            assert_eq!(entry, core::ptr::null_mut());
        }
    }

    #[test]
    fn heap_test_initial_state() {
        let test = HeapTest::new();

        let expected_list = [ListEntryDesc::new(0, test.size())];
        test.validate_free_list(&expected_list);
    }

    #[test]
    fn heap_test_allocate_u32() {
        let mut test = HeapTest::new();

        let layout = Layout::for_value(&30u32);
        let _ptr = unsafe { test.allocator.alloc(layout) };

        let expected_list = [ListEntryDesc::new(16, test.size() - 16)];
        test.validate_free_list(&expected_list);
    }

    #[test]
    fn heap_test_allocate_u64() {
        let mut test = HeapTest::new();

        let layout = Layout::for_value(&30u64);
        let _ptr = unsafe { test.allocator.alloc(layout) };

        let expected_list = [ListEntryDesc::new(16, test.size() - 16)];
        test.validate_free_list(&expected_list);
    }

    #[test]
    fn heap_test_allocate_136bits() {
        struct MyStruct(u128, u8);
        let mut test = HeapTest::new();

        let layout = Layout::new::<MyStruct>();
        let _ptr = unsafe { test.allocator.alloc(layout) };

        let expected_list = [ListEntryDesc::new(32, test.size() - 32)];
        test.validate_free_list(&expected_list);
    }

    #[test]
    fn heap_test_allocate_multiple() {
        struct MyStruct(u128, u8);
        let mut test = HeapTest::new();

        let layout = Layout::new::<MyStruct>();
        let _ptr = unsafe { test.allocator.alloc(layout) };

        let expected_list = [ListEntryDesc::new(32, test.size() - 32)];
        test.validate_free_list(&expected_list);

        let layout = Layout::new::<u32>();
        let _ptr = unsafe { test.allocator.alloc(layout) };

        let expected_list = [ListEntryDesc::new(48, test.size() - 48)];
        test.validate_free_list(&expected_list);
    }

    #[test]
    fn free_some_memory() {
        struct MyStruct(u128, u8);
        let mut test = HeapTest::new();

        let first_layout = Layout::new::<MyStruct>();
        let first_ptr = unsafe { test.allocator.alloc(first_layout) };

        let expected_list = [ListEntryDesc::new(32, test.size() - 32)];
        test.validate_free_list(&expected_list);

        let second_layout = Layout::new::<u32>();
        let second_ptr = unsafe { test.allocator.alloc(second_layout) };

        let expected_list = [ListEntryDesc::new(48, test.size() - 48)];
        test.validate_free_list(&expected_list);

        // Free the first allocation
        unsafe { test.allocator.dealloc(first_ptr, first_layout) };

        test.validate_free_list(&[
            ListEntryDesc::new(0, 32),
            ListEntryDesc::new(48, test.size() - 48),
        ]);

        // Free the second allocation
        unsafe { test.allocator.dealloc(second_ptr, second_layout) };

        test.validate_free_list(&[ListEntryDesc::new(0, test.size())]);
    }

    #[test]
    fn free_more_memory() {
        struct MyStruct(u128, u8);
        let mut test = HeapTest::new();

        let first_layout = Layout::new::<MyStruct>();
        let first_ptr = unsafe { test.allocator.alloc(first_layout) };

        let expected_list = [ListEntryDesc::new(32, test.size() - 32)];
        test.validate_free_list(&expected_list);

        let second_layout = Layout::new::<u32>();
        let second_ptr = unsafe { test.allocator.alloc(second_layout) };

        let expected_list = [ListEntryDesc::new(48, test.size() - 48)];
        test.validate_free_list(&expected_list);

        let third_layout = Layout::new::<u32>();
        let third_ptr = unsafe { test.allocator.alloc(third_layout) };

        let expected_list = [ListEntryDesc::new(64, test.size() - 64)];
        test.validate_free_list(&expected_list);

        let fourth_layout = Layout::new::<u32>();
        let fourth_ptr = unsafe { test.allocator.alloc(fourth_layout) };

        let expected_list = [ListEntryDesc::new(80, test.size() - 80)];
        test.validate_free_list(&expected_list);

        // Free the first allocation
        unsafe { test.allocator.dealloc(first_ptr, first_layout) };

        test.validate_free_list(&[
            ListEntryDesc::new(0, 32),
            ListEntryDesc::new(80, test.size() - 80),
        ]);

        // Free the third allocation
        unsafe { test.allocator.dealloc(third_ptr, third_layout) };

        test.validate_free_list(&[
            ListEntryDesc::new(0, 32),
            ListEntryDesc::new(48, 16),
            ListEntryDesc::new(80, test.size() - 80),
        ]);

        // Free the second allocation
        unsafe { test.allocator.dealloc(second_ptr, second_layout) };

        test.validate_free_list(&[
            ListEntryDesc::new(0, 64),
            ListEntryDesc::new(80, test.size() - 80),
        ]);

        // Free the last allocation
        unsafe { test.allocator.dealloc(fourth_ptr, fourth_layout) };

        test.validate_free_list(&[ListEntryDesc::new(0, test.size())]);
    }
}
