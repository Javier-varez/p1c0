use crate::{
    arch::{exceptions::ExceptionContext, mmu::PAGE_SIZE},
    elf::{self, ElfParser},
    memory::{
        self,
        address::{Address, VirtualAddress},
        address_space::{self, ProcessAddressSpace},
        num_pages_from_bytes,
        physical_page_allocator::PhysicalMemoryRegion,
        GlobalPermissions, MemoryManager, Permissions,
    },
    prelude::*,
    sync::spinlock::SpinLock,
    thread::{self, ThreadHandle},
};

use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug)]
pub enum Error {
    AddressSpaceError(address_space::Error),
    MemoryError(memory::Error),
    ThreadError(thread::Error),
    NoCurrentProcess,
    InvalidBase,
    ElfError(elf::Error),
    UnsupportedExecutable,
    UnalignedLoadableSegment,
    NoEntryPoint,
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
    arguments: Vec<String>,
    environment: FlatMap<String, String>,
    entrypoint: Option<VirtualAddress>,
    aslr_base: Option<VirtualAddress>,
    elf_data: Vec<u8>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            address_space: ProcessAddressSpace::new(),
            arguments: vec![],
            environment: FlatMap::new(),
            entrypoint: None,
            aslr_base: None,
            elf_data: vec![],
        }
    }
}

impl Builder {
    const STACK_SIZE: usize = 32 * 1024;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_entrypoint(&mut self, entrypoint: VirtualAddress) {
        self.entrypoint = Some(entrypoint);
    }

    pub fn set_elf_data(&mut self, elf_data: Vec<u8>) {
        self.elf_data = elf_data;
    }

    pub fn set_aslr_base(&mut self, aslr_base: VirtualAddress) {
        self.aslr_base = Some(aslr_base);
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
            MemoryManager::instance().do_with_fast_map(
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

        let num_pages = num_pages_from_bytes(size_bytes);
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

    pub fn push_argument(&mut self, arg: &str) {
        self.arguments.push(arg.to_string());
    }

    pub fn push_environment_variable(&mut self, key: &str, value: &str) {
        self.environment.insert(key.to_string(), value.to_string());
    }

    fn map_stack(&mut self, aslr_base: VirtualAddress) -> Result<VirtualAddress, Error> {
        let num_pages = num_pages_from_bytes(Self::STACK_SIZE);
        let pmr = MemoryManager::instance()
            .request_any_pages(num_pages, memory::AllocPolicy::ZeroFill)?;

        let stack_va =
            VirtualAddress::try_from_ptr((0xF00000000000 + aslr_base.as_u64()) as *const _)
                .map_err(|_e| Error::InvalidBase)?;
        self.address_space.map_section(
            ".stack",
            stack_va,
            pmr,
            Self::STACK_SIZE,
            GlobalPermissions::new_for_process(Permissions::RW),
        )?;
        Ok(stack_va)
    }

    fn map_arguments(
        &mut self,
        aslr_base: VirtualAddress,
    ) -> Result<(usize, VirtualAddress, VirtualAddress), Error> {
        let mut mapped_arg_addresses: Vec<*const u8> = vec![];
        let mut mapped_env_addresses: Vec<*const u8> = vec![];

        let args_va_start = unsafe {
            VirtualAddress::new_unchecked(0xF80000000000 as *const _).offset(aslr_base.as_usize())
        };
        // We are going to assume that args + environment fit in the PAGE_SIZE, which should REALLY be the case
        let pmr = MemoryManager::instance().request_any_pages(1, memory::AllocPolicy::ZeroFill)?;
        let pmr_base_address = pmr.base_address();

        self.address_space.map_section(
            ".args",
            args_va_start,
            pmr,
            PAGE_SIZE,
            GlobalPermissions::new_for_process(Permissions::RO),
        )?;

        Ok(MemoryManager::instance().do_with_fast_map(
            pmr_base_address,
            GlobalPermissions::new_only_privileged(Permissions::RW),
            |tmp_va| {
                let mut offset = 0;

                let mut copy_string = |str: &str| {
                    let len = str.len();
                    let va = unsafe { args_va_start.offset(offset) };

                    assert!((offset + len + 1) <= PAGE_SIZE);
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            str.as_ptr(),
                            tmp_va.offset(offset).as_mut_ptr(),
                            len,
                        );
                        offset += len;
                        core::ptr::write(tmp_va.offset(offset).as_mut_ptr(), 0);
                        offset += 1;
                    }
                    va
                };
                for arg in &self.arguments {
                    let va = copy_string(arg);
                    mapped_arg_addresses.push(va.as_ptr());
                }

                for (key, value) in self.environment.iter() {
                    let mut envvar = key.clone();
                    envvar.push('=');
                    envvar.push_str(value);

                    let va = copy_string(&envvar);
                    mapped_env_addresses.push(va.as_ptr());
                }

                // Now that the data is there, we need to push the arrays
                let argc = mapped_arg_addresses.len();

                mapped_arg_addresses.push(core::ptr::null());
                mapped_env_addresses.push(core::ptr::null());

                let mut copy_slice = |slice: &[*const u8]| {
                    let size_bytes = slice.len() * core::mem::size_of::<*const u8>();

                    // Align offset to pointer size
                    let alignment = offset % core::mem::size_of::<*const u8>();
                    if alignment != 0 {
                        offset += core::mem::size_of::<*const u8>() - alignment;
                    }

                    let va = unsafe { args_va_start.offset(offset) };

                    assert!((offset + size_bytes) <= PAGE_SIZE);
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            slice.as_ptr(),
                            tmp_va.offset(offset).as_mut_ptr() as *mut *const u8,
                            slice.len(),
                        );
                        offset += size_bytes;
                        core::ptr::write(tmp_va.offset(offset).as_mut_ptr(), 0);
                        offset += 1;
                    }
                    va
                };
                let argv = copy_slice(&mapped_arg_addresses);
                let envp = copy_slice(&mapped_env_addresses);
                (argc, argv, envp)
            },
        ))
    }

    pub fn start(mut self) -> Result<ProcessHandle, Error> {
        let entrypoint = self.entrypoint.ok_or(Error::NoEntryPoint)?;
        let aslr_base = self
            .aslr_base
            .unwrap_or_else(|| VirtualAddress::new_unaligned(core::ptr::null()));
        let stack_va = self.map_stack(aslr_base)?;
        let args = self.map_arguments(aslr_base)?;

        // Reserve PID
        let pid = NUM_PROCESSES.fetch_add(1, Ordering::Relaxed);

        let mut process = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(Process {
            address_space: self.address_space,
            thread_list: vec![],
            state: State::Running,
            pid,
            aslr_base,
            elf_data: self.elf_data,
        })));

        // Lock before we create threads or we might get preempted before the process is valid, but
        // the thread has a ref to it.
        let mut processes = PROCESSES.lock();

        let thread_id = thread::new_for_process(
            ProcessHandle(pid),
            stack_va,
            Self::STACK_SIZE,
            entrypoint,
            aslr_base,
            args,
        );
        process.thread_list.push(thread_id);

        processes.push(process);
        Ok(ProcessHandle(pid))
    }

    pub fn new_from_elf_data(name: &str, elf_data: Vec<u8>, aslr: usize) -> Result<Builder, Error> {
        let elf = ElfParser::from_slice(&elf_data[..]).map_err(|e| Error::ElfError(e))?;
        if !matches!(
            elf.elf_type(),
            elf::EType::Executable | elf::EType::SharedObject
        ) {
            log_warning!("Elf file is not executable, bailing");
            return Err(Error::UnsupportedExecutable);
        }

        let mut process_builder = Builder::new();
        for header in elf.program_header_iter() {
            let header_type = header.ty().map_err(|e| Error::ElfError(e))?;
            if matches!(header_type, elf::PtType::Load) {
                log_debug!(
                    "Virtual addr 0x{:x}, Physical addr 0x{:x}, Size in process {} Size in file {}",
                    header.vaddr(),
                    header.paddr(),
                    header.memsize(),
                    header.filesize()
                );

                let vaddr = (header.vaddr() as usize + aslr) as *const _;
                let vaddr = VirtualAddress::try_from_ptr(vaddr)
                    .map_err(|_| Error::UnalignedLoadableSegment)?;

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
                        .map_err(|e| Error::ElfError(e))?
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

        process_builder.set_aslr_base(VirtualAddress::new_unaligned(aslr as *const _));
        let vaddr = (elf.entry_point() as usize + aslr) as *const _;
        process_builder.set_entrypoint(VirtualAddress::new_unaligned(vaddr));
        process_builder.set_elf_data(elf_data);
        process_builder.push_argument(name);
        Ok(process_builder)
    }
}

pub struct Process {
    address_space: ProcessAddressSpace,
    // List of thread IDs of our threads
    thread_list: Vec<ThreadHandle>,
    state: State,
    pid: u64,
    aslr_base: VirtualAddress,
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
            aslr_base: self.aslr_base,
        }
    }

    pub fn exit_code(&self) -> Option<u64> {
        match self.state {
            State::Killed(return_value) => {
                // TODO(javier-varez): Reap process here somehow
                Some(return_value)
            }
            State::Running => None,
        }
    }
}

#[derive(Clone)]
pub struct ProcessSymbolicator<'a> {
    elf_parser: ElfParser<'a>,
    aslr_base: VirtualAddress,
}

impl<'a> crate::backtrace::Symbolicator for ProcessSymbolicator<'a> {
    fn symbolicate(&self, addr: VirtualAddress) -> Option<(String, usize)> {
        let addr = addr.remove_base(self.aslr_base).as_usize();

        self.elf_parser
            .symbol_table_iter()?
            .filter(|symbol| matches!(symbol.ty(), Ok(elf::SymbolType::Function)))
            .find_map(|symbol| {
                let symbol_start = symbol.value() as usize;
                let symbol_size = symbol.size() as usize;
                if (addr >= symbol_start) && (addr < (symbol_start + symbol_size)) {
                    symbol
                        .name()
                        .map(|string| (string.to_string(), addr - symbol_start))
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
        "Killing process with PID {}, exit code 0x{:x}",
        killed_proc.pid,
        error_code
    );

    thread::wake_threads_waiting_on_pid(&pid, error_code);
    thread::exit_matching_threads(&mut killed_proc.thread_list, cx)?;

    // Don't free process but instead keep it in a zombie state until states are collected
    killed_proc.state = State::Killed(error_code);
    Ok(())
}

pub(crate) fn validate_pid(pid: u64) -> Option<ProcessHandle> {
    PROCESSES
        .lock()
        .iter()
        .find(|process| process.pid == pid)
        .map(|process| ProcessHandle(process.pid))
}
