use crate::{
    arch::mmu::PAGE_SIZE,
    memory::{
        address::{Address, LogicalAddress, PhysicalAddress, VirtualAddress},
        physical_page_allocator::PhysicalMemoryRegion,
    },
    prelude::*,
};

use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
};

use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
    registers::InMemoryRegister,
};

register_bitfields! {u16,
    DescriptorFlags [
        Next OFFSET(0) NUMBITS(1) [],
        DEVICE_PERMISSIONS OFFSET(1) NUMBITS(1) [
            Readable = 0,
            Writeable = 1,
        ],
        DEVICE_INDIRECT OFFSET(2) NUMBITS(1) []
    ],
    AvailableFlags [
        NO_INTERRUPT OFFSET(0) NUMBITS(1) []
    ],
    UsedFlags [
        NO_NOTIFY OFFSET(0) NUMBITS(1) []
    ]
}

#[repr(C)]
struct Descriptor {
    addr: InMemoryRegister<u64>,
    len: InMemoryRegister<u32>,
    flags: InMemoryRegister<u16, DescriptorFlags::Register>,
    next: InMemoryRegister<u16>,
}

impl Descriptor {
    const fn new_empty() -> Self {
        Self {
            addr: InMemoryRegister::new(0),
            len: InMemoryRegister::new(0),
            flags: InMemoryRegister::new(0),
            next: InMemoryRegister::new(0),
        }
    }
}

#[repr(C)]
struct UsedElement {
    idx: InMemoryRegister<u32>,
    len: InMemoryRegister<u32>,
}

impl UsedElement {
    const fn new() -> Self {
        Self {
            idx: InMemoryRegister::new(0),
            len: InMemoryRegister::new(0),
        }
    }
}

#[repr(C, align(16))]
pub struct DescriptorTable<const N: usize> {
    descriptors: [Descriptor; N],
}

impl<const N: usize> DescriptorTable<N> {
    const fn new() -> Self {
        // In this case the descriptor is just const to copy it. Cannot really use lazy_static here
        #[allow(clippy::declare_interior_mutable_const)]
        const DESCRIPTOR: Descriptor = Descriptor::new_empty();
        Self {
            descriptors: [DESCRIPTOR; N],
        }
    }
}

#[repr(C, align(2))]
pub struct AvailableRing<const N: usize> {
    flags: InMemoryRegister<u16, AvailableFlags::Register>,
    idx: InMemoryRegister<u16>,
    ring: [InMemoryRegister<u16>; N],
    used_event: InMemoryRegister<u16>,
}

impl<const N: usize> AvailableRing<N> {
    const fn new() -> Self {
        // In this case the descriptor is just const to copy it. Cannot really use lazy_static here
        #[allow(clippy::declare_interior_mutable_const)]
        const IN_MEM_REG: InMemoryRegister<u16> = InMemoryRegister::new(0);
        Self {
            flags: InMemoryRegister::new(0),
            idx: InMemoryRegister::new(0),
            ring: [IN_MEM_REG; N],
            used_event: InMemoryRegister::new(0),
        }
    }
}

#[repr(C, align(4))]
pub struct UsedRing<const N: usize> {
    flags: InMemoryRegister<u16, UsedFlags::Register>,
    idx: InMemoryRegister<u16>,
    ring: [UsedElement; N],
    avail_event: InMemoryRegister<u16>,
}

impl<const N: usize> UsedRing<N> {
    const fn new() -> Self {
        // In this case the descriptor is just const to copy it. Cannot really use lazy_static here
        #[allow(clippy::declare_interior_mutable_const)]
        const USED_ELEMENT: UsedElement = UsedElement::new();
        Self {
            flags: InMemoryRegister::new(0),
            idx: InMemoryRegister::new(0),
            ring: [USED_ELEMENT; N],
            avail_event: InMemoryRegister::new(0),
        }
    }
}

#[repr(C)]
struct DescriptorBuffer<const C: usize>([u8; C]);

impl<const C: usize> DescriptorBuffer<C> {
    const fn new() -> Self {
        Self([0; C])
    }
}

impl<const C: usize> core::ops::Deref for DescriptorBuffer<C> {
    type Target = [u8; C];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const C: usize> core::ops::DerefMut for DescriptorBuffer<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[repr(C)]
struct VirtQueueImpl<const N: usize, const C: usize> {
    descriptor_table: DescriptorTable<N>,
    available_ring: AvailableRing<N>,
    used_ring: UsedRing<N>,
}

impl<const N: usize, const C: usize> VirtQueueImpl<N, C> {
    const fn new() -> Self {
        Self {
            descriptor_table: DescriptorTable::new(),
            available_ring: AvailableRing::new(),
            used_ring: UsedRing::new(),
        }
    }
}

// TODO(javier-varez): Need to impl Drop for VirtQueue in order to free the pages and not leak them
pub struct VirtQueue<const N: usize, const C: usize> {
    inner: Box<VirtQueueImpl<N, C>, DeviceMemoryAllocator>,
    current_desc_idx: u16,
    last_used_idx: u16,
    descriptor_data: Box<[DescriptorBuffer<C>; N]>,
}

impl<const N: usize, const C: usize> VirtQueue<N, C> {
    fn init_descriptors(&mut self) {
        for (desc, buffer) in self
            .inner
            .descriptor_table
            .descriptors
            .iter_mut()
            .zip(self.descriptor_data.iter_mut())
        {
            let buffer_pa =
                LogicalAddress::new_unaligned(buffer.as_mut_ptr() as *mut u8).into_physical();

            desc.addr.set(buffer_pa.as_u64());
            desc.len.set(buffer.len() as u32);
            desc.next.set(0);
            desc.flags.set(0);
        }
    }

    const DESC_BUFFER: DescriptorBuffer<C> = DescriptorBuffer::new();
    pub fn allocate() -> Self {
        let inner = Box::new_in(VirtQueueImpl::new(), DeviceMemoryAllocator());

        let mut queue = Self {
            inner,
            current_desc_idx: 0,
            last_used_idx: 0,
            descriptor_data: Box::new([Self::DESC_BUFFER; N]),
        };
        queue.init_descriptors();

        queue
    }

    pub fn add_desc_to_available_ring(&mut self, dsc_idx: usize) {
        let inner = &mut *self.inner;

        inner.available_ring.flags.set(0);

        let index = inner.available_ring.idx.get() as usize % N;
        inner.available_ring.ring[index].set(dsc_idx as u16);

        inner
            .available_ring
            .idx
            .set(inner.available_ring.idx.get().wrapping_add(1));
    }

    pub fn post_event(&mut self) {
        let idx = self.current_desc_idx;
        self.current_desc_idx = idx + 1;

        // Mark descriptor as writeable
        self.inner.descriptor_table.descriptors[idx as usize]
            .flags
            .write(DescriptorFlags::DEVICE_PERMISSIONS::Writeable);
        self.add_desc_to_available_ring(idx as usize);
    }

    // Returns the descriptor index and used len
    fn pop_event(&mut self) -> Option<usize> {
        let inner = &*self.inner;
        if self.last_used_idx >= inner.used_ring.idx.get() {
            return None;
        }

        let used_ev = &inner.used_ring.ring[self.last_used_idx as usize % N];

        let idx = used_ev.idx.get();

        self.last_used_idx = self.last_used_idx.wrapping_add(1);
        Some(idx as usize)
    }

    pub fn handle_events(&mut self, mut handler: impl FnMut(&[u8])) {
        while let Some(dsc_index) = self.pop_event() {
            let dsc = &self.descriptor_data[dsc_index];

            // For this to be truly safe we need to invalidate the cache here
            crate::arch::cache::invalidate_va_range(
                VirtualAddress::new_unaligned(dsc.as_ptr()),
                dsc.len(),
            );
            handler(&dsc.0);

            // We can add the desc back to the queue
            self.add_desc_to_available_ring(dsc_index);
        }
    }

    pub fn should_notify(&self) -> bool {
        self.inner.used_ring.flags.read(UsedFlags::NO_NOTIFY) == 0
    }

    pub fn descriptor_table(&self) -> PhysicalAddress {
        let mm = crate::memory::MemoryManager::instance();
        mm.translate_kernel_address(VirtualAddress::new_unaligned(
            &self.inner.descriptor_table as *const _ as *const _,
        ))
        .unwrap()
    }

    pub fn available_ring(&self) -> PhysicalAddress {
        let mm = crate::memory::MemoryManager::instance();
        mm.translate_kernel_address(VirtualAddress::new_unaligned(
            &self.inner.available_ring as *const _ as *const _,
        ))
        .unwrap()
    }

    pub fn used_ring(&self) -> PhysicalAddress {
        let mm = crate::memory::MemoryManager::instance();
        mm.translate_kernel_address(VirtualAddress::new_unaligned(
            &self.inner.used_ring as *const _ as *const _,
        ))
        .unwrap()
    }
}

// This is a horrible allocator, but sometimes you gotta do what you gotta do!
struct DeviceMemoryAllocator();

unsafe impl Allocator for DeviceMemoryAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let size = layout.size();
        let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        let mut mm = crate::memory::MemoryManager::instance();
        let pages = mm
            .request_any_pages(num_pages, crate::memory::AllocPolicy::None)
            .map_err(|_| AllocError)?;

        // TODO(javier-varez): Free pages if this operation fails to not leak them.
        let va = mm
            .map_io(
                "DevMemAlloc",
                pages.base_address(),
                pages.num_pages() * crate::arch::mmu::PAGE_SIZE,
            )
            .map_err(|_| AllocError)?;

        let slice = unsafe { core::slice::from_raw_parts_mut(va.as_mut_ptr(), size) };

        NonNull::new(slice as *mut [u8]).ok_or(AllocError)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let size = layout.size();
        let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

        let va = VirtualAddress::new_unaligned(ptr.as_ptr());

        let mut mm = crate::memory::MemoryManager::instance();
        let pa = mm.translate_kernel_address(va).unwrap();

        mm.release_pages(PhysicalMemoryRegion::new(pa, num_pages))
            .unwrap();
    }
}
