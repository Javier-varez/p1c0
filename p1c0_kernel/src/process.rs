extern crate alloc;

use crate::memory::address::{Address, VirtualAddress};
use crate::memory::{MemoryManager, Permissions};
use crate::{
    arch,
    collections::{
        intrusive_list::{IntrusiveItem, IntrusiveList},
        OwnedMutPtr,
    },
    log_debug,
    memory::{
        address_space::{self, ProcessAddressSpace},
        physical_page_allocator,
    },
    sync::spinlock::SpinLock,
    thread::{self, ThreadHandle},
};
use alloc::{boxed::Box, vec, vec::Vec};

use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub enum Error {
    AddressSpaceError(address_space::Error),
    PageAllocError(physical_page_allocator::Error),
    InvalidBase,
}

impl From<address_space::Error> for Error {
    fn from(e: address_space::Error) -> Self {
        Error::AddressSpaceError(e)
    }
}

impl From<physical_page_allocator::Error> for Error {
    fn from(e: physical_page_allocator::Error) -> Self {
        Error::PageAllocError(e)
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
        let mut addr_space = ProcessAddressSpace::new();
        // Temporarily map this address space
        unsafe {
            arch::mmu::MMU.switch_process_translation_table(addr_space.address_table());
        }

        Self {
            address_space: addr_space,
        }
    }
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
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
        let mut data_len = size_bytes;
        if data.len() > data_len {
            data_len = data.len();
        }
        let num_pages = crate::memory::num_pages_from_bytes(data_len);
        let pmr = MemoryManager::instance()
            .page_allocator()
            .request_any_pages(num_pages)?;

        self.address_space
            .map_section(name, va, pmr, data_len, permissions)?;

        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), va.as_mut_ptr(), data.len());
        }

        Ok(())
    }

    pub fn start(
        mut self,
        entry_point: VirtualAddress,
        base_address: VirtualAddress,
    ) -> Result<(), Error> {
        // Allocate stack
        let pmr = MemoryManager::instance()
            .page_allocator()
            .request_any_pages(1)?;

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
