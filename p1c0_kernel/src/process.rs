extern crate alloc;

use crate::memory::address::{Address, VirtualAddress};
use crate::memory::{GlobalPermissions, MemoryManager, Permissions};
use crate::prelude::*;
use crate::{
    collections::{
        intrusive_list::{IntrusiveItem, IntrusiveList},
        OwnedMutPtr,
    },
    memory,
    memory::address_space::{self, ProcessAddressSpace},
    sync::spinlock::SpinLock,
    thread::{self, ThreadHandle},
};
use alloc::borrow::ToOwned;
use alloc::{boxed::Box, vec, vec::Vec};

use crate::arch::exceptions::ExceptionContext;
use crate::arch::mmu::PAGE_SIZE;
use crate::elf::{self, ElfParser};
use crate::memory::physical_page_allocator::PhysicalMemoryRegion;
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub enum Error {
    AddressSpaceError(address_space::Error),
    MemoryError(memory::Error),
    ThreadError(thread::Error),
    NoCurrentProcess,
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

impl From<thread::Error> for Error {
    fn from(e: thread::Error) -> Self {
        Error::ThreadError(e)
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
            memory::MemoryManager::instance().do_with_fast_map(
                pa,
                GlobalPermissions::new_only_privileged(Permissions::RW),
                |va| unsafe {
                    core::ptr::copy_nonoverlapping(
                        page_data.as_ptr(),
                        va.as_mut_ptr(),
                        page_data.len(),
                    );
                },
            );

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

        self.address_space.map_section(
            name,
            va,
            pmr,
            size_bytes,
            GlobalPermissions::new_for_process(permissions),
        )?;

        Ok(())
    }

    pub fn start(
        mut self,
        entry_point: VirtualAddress,
        base_address: VirtualAddress,
        elf_data: Vec<u8>,
    ) -> Result<(), Error> {
        // Allocate stack
        let pmr = MemoryManager::instance().request_any_pages(1, memory::AllocPolicy::ZeroFill)?;

        const STACK_SIZE: usize = 4096;

        let stack_va =
            VirtualAddress::try_from_ptr((0xF00000000000 + base_address.as_u64()) as *const _)
                .map_err(|_e| Error::InvalidBase)?;
        self.address_space.map_section(
            ".stack",
            stack_va,
            pmr,
            STACK_SIZE,
            GlobalPermissions::new_for_process(Permissions::RW),
        )?;

        let pid = NUM_PROCESSES.fetch_add(1, Ordering::Relaxed);

        let thread_id = thread::new_for_process(
            ProcessHandle(pid),
            stack_va,
            STACK_SIZE,
            entry_point,
            base_address,
        );

        let process = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(Process {
            address_space: self.address_space,
            thread_list: vec![thread_id],
            _state: State::Running,
            pid,
            base_address,
            elf_data,
        })));

        PROCESSES.lock().push(process);
        Ok(())
    }
}

pub struct Process {
    address_space: ProcessAddressSpace,
    // List of thread IDs of our threads
    thread_list: Vec<ThreadHandle>,
    _state: State,
    pid: u64,
    base_address: VirtualAddress,
    elf_data: Vec<u8>,
}

impl Process {
    pub fn address_space(&mut self) -> &mut ProcessAddressSpace {
        &mut self.address_space
    }

    pub fn symbolicator(&self) -> ProcessSymbolicator<'_> {
        let elf_parser = ElfParser::from_slice(&self.elf_data[..]).unwrap();
        ProcessSymbolicator {
            elf_parser,
            base_address: self.base_address,
        }
    }
}

#[derive(Clone)]
pub struct ProcessSymbolicator<'a> {
    elf_parser: ElfParser<'a>,
    base_address: VirtualAddress,
}

impl<'a> crate::backtrace::Symbolicator for ProcessSymbolicator<'a> {
    fn symbolicate(&self, addr: VirtualAddress) -> Option<(String, usize)> {
        let addr = addr.remove_base(self.base_address).as_usize();

        self.elf_parser
            .symbol_table_iter()?
            .filter(|symbol| matches!(symbol.ty(), Ok(elf::SymbolType::Function)))
            .find_map(|symbol| {
                let symbol_start = symbol.value() as usize;
                let symbol_size = symbol.size() as usize;
                if (addr >= symbol_start) && (addr < (symbol_start + symbol_size)) {
                    symbol
                        .name()
                        .map(|string| (string.to_owned(), addr - symbol_start))
                } else {
                    None
                }
            })
    }
}

pub(crate) fn do_with_process<T>(
    handle: &ProcessHandle,
    mut f: impl FnMut(&mut Process) -> T,
) -> T {
    let mut processes = PROCESSES.lock();
    let proc = processes
        .iter_mut()
        .find(|proc| proc.pid == handle.0)
        .expect("There isn't a matching process");
    f(proc)
}

pub(crate) fn kill_current_process(cx: &mut ExceptionContext) -> Result<(), Error> {
    let pid = match thread::current_pid() {
        Some(pid) => pid,
        None => {
            log_error!("Cannot kill current process. No process is currently running");
            return Err(Error::NoCurrentProcess);
        }
    };
    let mut removed_process = PROCESSES.lock().drain_filter(|p| p.pid == pid.0);

    // Only one process must match
    assert_eq!(removed_process.len(), 1);

    // # Safety: into_box is safe because processes are allocated with box
    let mut process = unsafe { removed_process.pop().unwrap().into_box() };
    log_info!("Killing process with PID {}", process.pid);

    crate::thread::exit_matching_threads(&mut process.thread_list, cx)?;
    Ok(())
}
