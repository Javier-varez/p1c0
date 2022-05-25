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
    ElfError(crate::elf::Error),
    UnsupportedExecutable,
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
    Killed(u64),
}

static NUM_PROCESSES: AtomicU64 = AtomicU64::new(0);

static PROCESSES: SpinLock<IntrusiveList<Process>> = SpinLock::new(IntrusiveList::new());

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessHandle(u64);

impl ProcessHandle {
    pub fn get_raw(&self) -> u64 {
        self.0
    }
}

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
    ) -> Result<ProcessHandle, Error> {
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

        let mut process = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(Process {
            address_space: self.address_space,
            thread_list: vec![],
            state: State::Running,
            pid,
            base_address,
            elf_data,
        })));

        // Lock before we create threads or we might get preempted before the process is valid, but
        // the thread has a ref to it.
        let mut processes = PROCESSES.lock();

        let thread_id = thread::new_for_process(
            ProcessHandle(pid),
            stack_va,
            STACK_SIZE,
            entry_point,
            base_address,
        );
        process.thread_list.push(thread_id);

        processes.push(process);
        Ok(ProcessHandle(pid))
    }
}

pub struct Process {
    address_space: ProcessAddressSpace,
    // List of thread IDs of our threads
    thread_list: Vec<ThreadHandle>,
    state: State,
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

    pub fn exit_code(&self) -> Option<u64> {
        match self.state {
            State::Killed(retval) => {
                // TODO(javier-varez): Reap process here somehow
                Some(retval)
            }
            State::Running => None,
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

pub(crate) fn kill_current_process(
    cx: &mut ExceptionContext,
    error_code: u64,
) -> Result<(), Error> {
    let pid = match thread::current_pid() {
        Some(pid) => pid,
        None => {
            log_error!("Cannot kill current process. No process is currently running");
            return Err(Error::NoCurrentProcess);
        }
    };
    let mut processes = PROCESSES.lock();

    let killed_proc = processes.iter_mut().find(|p| p.pid == pid.0).unwrap();

    log_info!(
        "Killing process with PID {}, exit code {}",
        killed_proc.pid,
        error_code
    );

    thread::wake_threads_waiting_on_pid(&pid, error_code);
    thread::exit_matching_threads(&mut killed_proc.thread_list, cx)?;

    // Don't free process but instead keep it in a zombie state unitl states are collected
    killed_proc.state = State::Killed(error_code);
    Ok(())
}

pub fn new_from_elf_data(elf_data: Vec<u8>, aslr: usize) -> Result<ProcessHandle, Error> {
    let elf =
        elf::ElfParser::from_slice(&elf_data[..]).map_err(|_| Error::UnsupportedExecutable)?;
    if !matches!(
        elf.elf_type(),
        elf::EType::Executable | elf::EType::SharedObject
    ) {
        log_warning!("Elf file is not executable, bailing");
        return Err(Error::UnsupportedExecutable);
    }

    let mut process_builder = Builder::new();
    for header in elf.program_header_iter() {
        let header_type = header.ty().map_err(|_| Error::UnsupportedExecutable)?;
        if matches!(header_type, elf::PtType::Load) {
            log_debug!(
                "Vaddr 0x{:x}, Paddr 0x{:x}, Memsize {} Filesize {}",
                header.vaddr(),
                header.paddr(),
                header.memsize(),
                header.filesize()
            );

            let vaddr = (header.vaddr() as usize + aslr) as *const _;
            let vaddr =
                VirtualAddress::try_from_ptr(vaddr).map_err(|_| Error::UnsupportedExecutable)?;

            let segment_data = elf.get_segment_data(&header);

            let permissions = match header.permissions() {
                elf::Permissions {
                    read: true,
                    write: true,
                    exec: false,
                } => Permissions::RW,
                elf::Permissions {
                    read: true,
                    write: false,
                    exec: false,
                } => Permissions::RO,
                elf::Permissions {
                    read: _,
                    write: false,
                    exec: true,
                } => Permissions::RX,
                elf::Permissions {
                    read: true,
                    write: true,
                    exec: true,
                } => Permissions::RWX,
                elf::Permissions { read, write, exec } => {
                    let read = if read { "R" } else { "-" };
                    let write = if write { "W" } else { "-" };
                    let exec = if exec { "X" } else { "-" };
                    panic!(
                        "Unsupported set of permissions found in elf {}{}{}",
                        read, write, exec
                    );
                }
            };

            process_builder.map_section(
                elf.matching_section_name(&header)
                    .map_err(|_| Error::UnsupportedExecutable)?
                    .unwrap_or(""),
                vaddr,
                header.memsize() as usize,
                segment_data,
                permissions,
            )?;
        } else {
            log_warning!("Unhandled ELF program header with type {:?}", header_type);
        }
    }

    let vaddr = (elf.entry_point() as usize + aslr) as *const _;
    let entry_point = VirtualAddress::new_unaligned(vaddr);
    let base_address = VirtualAddress::new_unaligned(aslr as *const _);
    process_builder.start(entry_point, base_address, elf_data)
}

pub(crate) fn validate_pid(pid: u64) -> Option<ProcessHandle> {
    PROCESSES
        .lock()
        .iter()
        .find(|process| process.pid == pid)
        .map(|process| ProcessHandle(process.pid))
}
