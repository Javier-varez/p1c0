extern crate alloc;

use crate::memory::address::{Address, VirtualAddress};
use crate::memory::{MemoryManager, Permissions};
use crate::{
    collections::{
        intrusive_list::{IntrusiveItem, IntrusiveList},
        OwnedMutPtr,
    },
    log_debug, memory,
    memory::address_space::{self, ProcessAddressSpace},
    sync::spinlock::SpinLock,
    thread::{self, ThreadHandle},
};
use alloc::{boxed::Box, vec, vec::Vec};

use crate::arch::mmu::PAGE_SIZE;
use crate::memory::physical_page_allocator::PhysicalMemoryRegion;
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub enum Error {
    AddressSpaceError(address_space::Error),
    MemoryError(memory::Error),
    InvalidBase,
}

impl From<address_space::Error> for Error {
    fn from(e: address_space::Error) -> Self {
        Error::AddressSpaceError(e)
    }
}

impl From<memory::Error> for Error {
    fn from(e: memory::Error) -> Self {
        Error::MemoryError(e)
    }
}

pub enum State {
    Running,
    Done,
}

static NUM_PROCESSES: AtomicU64 = AtomicU64::new(0);

static PROCESSES: SpinLock<IntrusiveList<Process>> = SpinLock::new(IntrusiveList::new());

#[derive(Clone, Debug)]
pub struct ProcessHandle(u64);

pub struct Builder {
    address_space: ProcessAddressSpace,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            address_space: ProcessAddressSpace::new(),
        }
    }
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    fn copy_section(&mut self, pmr: &PhysicalMemoryRegion, data: &[u8]) {
        // Initialize the physical page
        let mut remaining_bytes = data.len();
        let mut current_offset = 0;

        for i in 0..pmr.num_pages() {
            let pa = unsafe { pmr.base_address().offset(i * PAGE_SIZE) };
            let chunk_size = if remaining_bytes >= PAGE_SIZE {
                PAGE_SIZE
            } else {
                remaining_bytes
            };

            let page_data = &data[current_offset..];

            // Try to perform a fast mapping of the page to load the contents
            memory::MemoryManager::instance().do_with_fast_map(pa, Permissions::RW, |va| unsafe {
                core::ptr::copy_nonoverlapping(
                    page_data.as_ptr(),
                    va.as_mut_ptr(),
                    page_data.len(),
                );
            });

            remaining_bytes -= chunk_size;
            current_offset += chunk_size;
        }

        assert_eq!(remaining_bytes, 0);
        assert_eq!(current_offset, data.len());
    }

    pub fn map_section(
        &mut self,
        name: &str,
        va: VirtualAddress,
        size_bytes: usize,
        data: &[u8],
        permissions: Permissions,
    ) -> Result<(), Error> {
        log_debug!("Mapping section `{}` for new process", name);

        // TODO(javier-varez): In reality this should be done lazily in most cases
        assert!(size_bytes >= data.len());

        let num_pages = crate::memory::num_pages_from_bytes(size_bytes);
        let pmr = MemoryManager::instance()
            .request_any_pages(num_pages, memory::AllocPolicy::ZeroFill)?;

        self.copy_section(&pmr, data);

        self.address_space
            .map_section(name, va, pmr, size_bytes, permissions)?;

        Ok(())
    }

    pub fn start(
        mut self,
        entry_point: VirtualAddress,
        base_address: VirtualAddress,
    ) -> Result<(), Error> {
        // Allocate stack
        let pmr = MemoryManager::instance().request_any_pages(1, memory::AllocPolicy::ZeroFill)?;

        let stack_va =
            VirtualAddress::try_from_ptr((0xF00000000000 + base_address.as_u64()) as *const _)
                .map_err(|_e| Error::InvalidBase)?;
        self.address_space
            .map_section(".stack", stack_va, pmr, 4096, Permissions::RW)?;

        let pid = NUM_PROCESSES.fetch_add(1, Ordering::Relaxed);

        let thread_id = thread::new_for_process(
            ProcessHandle(pid),
            unsafe { stack_va.offset(4096) },
            entry_point,
            base_address,
        );

        let process = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(Process {
            address_space: self.address_space,
            _thread_list: vec![thread_id],
            _state: State::Running,
            pid,
        })));

        PROCESSES.lock().push(process);
        Ok(())
    }
}

pub struct Process {
    address_space: ProcessAddressSpace,
    // List of thread IDs of our threads
    _thread_list: Vec<ThreadHandle>,
    _state: State,
    pid: u64,
}

impl Process {
    pub fn address_space(&mut self) -> &mut ProcessAddressSpace {
        &mut self.address_space
    }
}

pub(crate) fn do_with_process(handle: &ProcessHandle, mut f: impl FnMut(&mut Process)) {
    for process in PROCESSES.lock().iter_mut() {
        if process.pid == handle.0 {
            f(process);
        }
    }
}
