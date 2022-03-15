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
    memory::address_space::ProcessAddressSpace,
    sync::spinlock::SpinLock,
    thread::{self, ThreadHandle},
};
use alloc::{boxed::Box, vec, vec::Vec};

use crate::memory::physical_page_allocator::Error;
use core::sync::atomic::{AtomicU64, Ordering};

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

impl Builder {
    pub fn new() -> Self {
        let mut addr_space = ProcessAddressSpace::new();
        // Temporarily map this address space
        unsafe {
            arch::mmu::MMU.switch_process_translation_table(addr_space.address_table());
        }

        Self {
            address_space: addr_space,
        }
    }

    pub fn map_section(
        mut self,
        name: &str,
        va: VirtualAddress,
        data: &[u8],
        permissions: Permissions,
    ) -> Self {
        log_debug!("Mapping section `{}` for new process", name);
        let num_pages = crate::memory::num_pages_from_bytes(data.len());
        let pmr = MemoryManager::instance()
            .page_allocator()
            .request_any_pages(num_pages)
            .unwrap();

        self.address_space
            .map_section(name, va, pmr, data.len(), permissions)
            .unwrap();

        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), va.as_mut_ptr(), data.len());
        }

        self
    }

    pub fn start(mut self, entry_point: VirtualAddress) -> Result<(), Error> {
        // Allocate stack
        let pmr = MemoryManager::instance()
            .page_allocator()
            .request_any_pages(1)
            .unwrap();

        let stack_va = VirtualAddress::try_from_ptr(0x0000000100000000 as *const _).unwrap();
        self.address_space
            .map_section(".stack", stack_va, pmr, 4096, Permissions::RW)
            .unwrap();

        let pid = NUM_PROCESSES.fetch_add(1, Ordering::Relaxed);

        let thread_id = thread::new_for_process(
            ProcessHandle(pid),
            unsafe { stack_va.offset(4096) },
            entry_point,
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