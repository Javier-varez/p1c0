extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use arch::StackType;
use cortex_a::{asm::wfi, registers::SPSR_EL1};
use heapless::String;
use tock_registers::interfaces::Readable;

use crate::{
    arch,
    arch::exceptions::ExceptionContext,
    collections::{
        intrusive_list::{IntrusiveItem, IntrusiveList},
        OwnedMutPtr,
    },
    drivers::interfaces::{timer::Timer, Ticks},
    memory::address,
    prelude::*,
    sync::spinlock::SpinLock,
};

use crate::arch::exceptions::return_from_exception;
use crate::drivers::generic_timer::get_timer;
use crate::memory::address::{Address, VirtualAddress};
use crate::process::{do_with_process, ProcessHandle};
use crate::syscall::Syscall;
use core::ops::Add;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;

#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    ThreadNotFound,
}

enum Stack {
    KernelThread(Vec<u64>),
    ProcessThread(VirtualAddress, usize /* num_pages */),
}

impl Stack {
    fn new(size: usize) -> Self {
        let mut stack: Vec<u64> = Vec::with_capacity(size);
        #[allow(clippy::uninit_vec)]
        unsafe {
            stack.set_len(size)
        };
        Self::KernelThread(stack)
    }

    fn top(&self) -> u64 {
        match &self {
            Stack::KernelThread(stack) => &stack[stack.len() - 1] as *const _ as u64,
            Stack::ProcessThread(va, size) => va.as_u64() + *size as u64,
        }
    }

    fn base(&self) -> VirtualAddress {
        match &self {
            Stack::KernelThread(stack) => VirtualAddress::new_unaligned(stack.as_ptr() as *const _),
            Stack::ProcessThread(stack, _) => *stack,
        }
    }

    fn len(&self) -> usize {
        match &self {
            Stack::KernelThread(stack) => stack.len(),
            Stack::ProcessThread(_, size) => *size,
        }
    }

    fn validator(&self) -> StackValidator {
        StackValidator {
            range_base: self.base(),
            range_len: self.len(),
        }
    }
}

#[derive(Clone)]
pub struct StackValidator {
    range_base: VirtualAddress,
    range_len: usize,
}

impl address::Validator for StackValidator {
    fn is_valid(&self, ptr: address::VirtualAddress) -> bool {
        let range_base = self.range_base.as_usize();
        let range_len = self.range_len;

        let ptr = ptr.as_usize();
        (ptr >= range_base) || (ptr < (range_base + range_len))
    }
}

enum BlockReason {
    Sleep(Ticks),
    Join(ThreadHandle),
    WaitForPid(ProcessHandle),
}

pub struct ThreadControlBlock {
    tid: u64,
    name: String<32>,
    process: Option<crate::process::ProcessHandle>,
    entry: Option<Box<dyn FnOnce()>>,
    stack: Stack,
    is_idle_thread: bool,

    // Blocking conditions
    block_reason: Option<BlockReason>,

    // Context switch data
    regs: [u64; 31],
    elr: u64,
    spsr: u64,
    stack_ptr: u64,
}

impl ThreadControlBlock {
    pub fn name(&self) -> Option<&str> {
        if self.name.is_empty() {
            None
        } else {
            Some(&self.name)
        }
    }
}

type Tcb = OwnedMutPtr<IntrusiveItem<ThreadControlBlock>>;

static ACTIVE_THREADS: SpinLock<IntrusiveList<ThreadControlBlock>> =
    SpinLock::new(IntrusiveList::new());

static BLOCKED_THREADS: SpinLock<IntrusiveList<ThreadControlBlock>> =
    SpinLock::new(IntrusiveList::new());

static CURRENT_THREAD: SpinLock<Option<Tcb>> = SpinLock::new(None);
static IDLE_THREAD: SpinLock<Option<Tcb>> = SpinLock::new(None);

static NUM_THREADS: AtomicU64 = AtomicU64::new(0);

extern "C" fn thread_start(thread_control_block: &mut ThreadControlBlock) {
    match thread_control_block.entry.take() {
        Some(closure) => closure(),
        _ => panic!("Expected to find Entry::Start"),
    };
}

pub struct ThreadHandle(u64);

impl ThreadHandle {
    pub fn join(self) {
        Syscall::thread_join(self.0);
    }
}

pub struct Builder {
    name: Option<String<32>>,
    stack_size: Option<usize>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub const fn new() -> Self {
        Self {
            name: None,
            stack_size: None,
        }
    }

    #[must_use]
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(String::from(name));
        self
    }

    #[must_use]
    pub fn stack_size(mut self, size: usize) -> Self {
        self.stack_size = Some(size);
        self
    }

    fn create<F>(self, thread: F) -> Tcb
    where
        F: FnOnce() + Send + 'static,
    {
        let thread_wrapper = Box::new(move || {
            thread();

            Syscall::thread_exit();
        });

        const DEFAULT_STACK_SIZE: usize = 1024;

        let name = self.name.unwrap_or_else(String::new);
        let stack_size = self.stack_size.unwrap_or(DEFAULT_STACK_SIZE);
        let stack = Stack::new(stack_size);
        let stack_ptr = stack.top();
        let elr = thread_start as usize;
        let mut spsr = SPSR_EL1.extract();
        spsr.write(SPSR_EL1::M::EL1t);
        let regs = [0; 31];
        let tid = NUM_THREADS.fetch_add(1, Ordering::Relaxed);

        let mut tcb = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(ThreadControlBlock {
            tid,
            name,
            entry: Some(thread_wrapper),
            stack,
            process: None,
            block_reason: None,
            regs,
            elr: elr as u64,
            spsr: spsr.get(),
            stack_ptr,
            is_idle_thread: false,
        })));
        tcb.regs[0] = (&mut **tcb) as *mut ThreadControlBlock as u64;

        tcb
    }

    pub fn spawn<F>(self, thread: F) -> ThreadHandle
    where
        F: FnOnce() + Send + 'static,
    {
        let tcb = self.create(thread);
        let tid = tcb.tid;
        ACTIVE_THREADS.lock().push(tcb);
        ThreadHandle(tid)
    }
}

pub fn spawn<F>(thread: F) -> ThreadHandle
where
    F: FnOnce() + Send + 'static,
{
    Builder::new().spawn(thread)
}

pub(crate) fn new_for_process(
    process: ProcessHandle,
    stack_va: VirtualAddress,
    stack_size: usize,
    entry_point: VirtualAddress,
    base_address: VirtualAddress,
    (argc, argv, envp): (usize, VirtualAddress, VirtualAddress),
) -> ThreadHandle {
    let name = String::new();
    let stack = Stack::ProcessThread(stack_va, stack_size);
    let stack_ptr = stack.top();
    let elr = entry_point.as_ptr();
    let mut spsr = SPSR_EL1.extract();
    spsr.write(SPSR_EL1::M::EL0t);
    let regs = [0; 31];
    let tid = NUM_THREADS.fetch_add(1, Ordering::Relaxed);

    let mut tcb = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(ThreadControlBlock {
        tid,
        name,
        entry: None,
        stack,
        process: Some(process),
        block_reason: None,
        regs,
        elr: elr as u64,
        spsr: spsr.get(),
        stack_ptr,
        is_idle_thread: false,
    })));
    tcb.regs[0] = argc as u64;
    tcb.regs[1] = argv.as_u64();
    tcb.regs[2] = envp.as_u64();
    tcb.regs[3] = base_address.as_u64();

    ACTIVE_THREADS.lock().push(tcb);

    ThreadHandle(tid)
}

pub fn initialize() -> ! {
    let mut current_thread = CURRENT_THREAD.lock();
    assert!(current_thread.is_none());

    // Spawn idle thread
    let mut idle = Builder::new().name("Idle").stack_size(128).create(|| loop {
        wfi();
    });
    idle.is_idle_thread = true;
    IDLE_THREAD.lock().replace(idle);

    // Let's take the first element in the thread list and run that
    let thread = ACTIVE_THREADS.lock().pop().expect("No threads found!");
    current_thread.replace(thread);

    let tcb = current_thread.as_ref().unwrap();

    // TODO(javier-varez): This should be a regular context switch or otherwise there are no guarantees on the value of registers on entry...
    let mut cx = ExceptionContext::default();
    restore_thread_context(&mut cx, tcb);
    drop(current_thread);

    return_from_exception(cx);
}

fn save_thread_context(thread: &mut Tcb, cx: &ExceptionContext) {
    thread.spsr = cx.spsr_el1.as_raw();
    thread.stack_ptr = cx.sp_el0;
    thread.regs.copy_from_slice(&cx.gpr[..]);
    thread.elr = cx.elr_el1;
}

fn restore_thread_context(cx: &mut ExceptionContext, thread: &Tcb) {
    cx.spsr_el1.from_raw(thread.spsr);
    cx.sp_el0 = thread.stack_ptr;
    cx.gpr.copy_from_slice(&thread.regs[..]);
    cx.elr_el1 = thread.elr;

    if let Some(handle) = thread.process.as_ref() {
        do_with_process(handle, |process| {
            arch::mmu::switch_process_translation_table(process.address_space().address_table());
        });
    } else {
        // Set the kernel translation table instead
        crate::memory::MemoryManager::instance().map_kernel_low_pages();
    }
}

fn wake_asleep_threads() {
    let current_ticks = get_timer().ticks();
    let unblocked_threads = BLOCKED_THREADS.lock().drain_filter(|thread| {
        if let BlockReason::Sleep(ticks) = thread.block_reason.as_ref().unwrap() {
            return *ticks <= current_ticks;
        }
        false
    });

    ACTIVE_THREADS.lock().join(unblocked_threads);
}

pub(crate) fn wake_threads_waiting_on_pid(pid: &ProcessHandle, exit_code: u64) {
    let mut unblocked_threads = BLOCKED_THREADS.lock().drain_filter(|thread| {
        if let BlockReason::WaitForPid(p) = thread.block_reason.as_ref().unwrap() {
            return p == pid;
        }
        false
    });

    // Set the exit code in those threads
    unblocked_threads.iter_mut().for_each(|thread| {
        thread.regs[0] = exit_code;
    });

    ACTIVE_THREADS.lock().join(unblocked_threads);
}

fn schedule_next_thread() -> Tcb {
    wake_asleep_threads();

    // This is the actual round-robin scheduling algo... For now it works, but it is obviously not
    // optimal
    ACTIVE_THREADS
        .lock()
        .pop()
        .unwrap_or_else(|| IDLE_THREAD.lock().take().unwrap())
}

pub fn run_scheduler(cx: &mut ExceptionContext) {
    // This should run scheduler and perform context switch.
    // At this point the simplest form of round robin scheduling is implemented.

    let mut current_thread = CURRENT_THREAD.lock();

    let mut thread = match current_thread.take() {
        Some(thread) => thread,
        None => {
            // Scheduler is not started
            return;
        }
    };

    save_thread_context(&mut thread, cx);

    if thread.is_idle_thread {
        IDLE_THREAD.lock().replace(thread);
    } else {
        // Store the thread in the list again
        ACTIVE_THREADS.lock().push(thread);
    }

    let thread = schedule_next_thread();
    restore_thread_context(cx, &thread);
    current_thread.replace(thread);
}

pub fn sleep_current_thread(cx: &mut ExceptionContext, duration: Duration) {
    let mut current_thread = CURRENT_THREAD.lock();

    let mut thread = current_thread
        .take()
        .expect("There is no current thread calling sleep!");
    assert!(!thread.is_idle_thread);

    save_thread_context(&mut thread, cx);

    // Compute wakeup ticks
    let timer = get_timer();
    let timer_res = get_timer().resolution();

    let current_ticks = timer.ticks();

    let time_since_epoch = timer_res.ticks_to_duration(current_ticks);
    let target_ticks = timer_res.duration_to_ticks(time_since_epoch.add(duration));

    thread.block_reason = Some(BlockReason::Sleep(target_ticks));
    BLOCKED_THREADS.lock().push(thread);

    let thread = schedule_next_thread();
    restore_thread_context(cx, &thread);
    current_thread.replace(thread);
}

fn exit_thread(thread: Tcb) {
    let tid = thread.tid;

    // Drop the thread
    unsafe { thread.into_box() };

    // Get the TID and unlock any threads that were waiting for this one to complete
    let unblocked_threads = BLOCKED_THREADS.lock().drain_filter(|thread| {
        if let BlockReason::Join(handle) = thread.block_reason.as_ref().unwrap() {
            return handle.0 == tid;
        }
        false
    });
    ACTIVE_THREADS.lock().join(unblocked_threads);
}

pub fn exit_current_thread(cx: &mut ExceptionContext) {
    let mut current_thread = CURRENT_THREAD.lock();

    let thread = current_thread
        .take()
        .expect("There is no current thread calling sleep!");
    assert!(!thread.is_idle_thread);

    // Exit the thread
    exit_thread(thread);

    let thread = schedule_next_thread();
    restore_thread_context(cx, &thread);
    current_thread.replace(thread);
}

fn validate_thread_handle(tid: u64) -> bool {
    // TODO(javier-varez): This could be made way more efficient than a linear search in two
    // containers.
    if ACTIVE_THREADS.lock().iter().any(|thread| thread.tid == tid) {
        return true;
    }

    if BLOCKED_THREADS
        .lock()
        .iter()
        .any(|thread| thread.tid == tid)
    {
        return true;
    }

    false
}

pub fn join_thread(cx: &mut ExceptionContext, tid: u64) {
    if !validate_thread_handle(tid) {
        // TODO(javier-varez): Should return an error here
        return;
    }

    let mut current_thread = CURRENT_THREAD.lock();

    let mut thread = current_thread
        .take()
        .expect("There is no current thread calling sleep!");
    assert!(!thread.is_idle_thread);

    save_thread_context(&mut thread, cx);

    thread.block_reason = Some(BlockReason::Join(ThreadHandle(tid)));
    BLOCKED_THREADS.lock().push(thread);

    let thread = schedule_next_thread();
    restore_thread_context(cx, &thread);
    current_thread.replace(thread);
}

pub fn print_thread_info() {
    let current_thread = CURRENT_THREAD.lock();
    let threads = ACTIVE_THREADS.lock();
    let blocked_threads = BLOCKED_THREADS.lock();

    log_info!("Thread information:");
    if let Some(tcb) = &*current_thread {
        if let Some(name) = tcb.name() {
            log_info!("\tCurrent thread: {}, tid: {}", name, tcb.tid);
        } else {
            log_info!("\tCurrent thread tid: {}", tcb.tid);
        }
    }

    for tcb in threads.iter() {
        if let Some(name) = tcb.name() {
            log_info!("\tThread: {}, tid: {}", name, tcb.tid);
        } else {
            log_info!("\tAnonymous thread, tid: {}", tcb.tid);
        }
    }

    for tcb in blocked_threads.iter() {
        if let Some(name) = tcb.name() {
            log_info!("\tBlocked thread: {}, tid: {}", name, tcb.tid);
        } else {
            log_info!("\tAnonymous blocked thread, tid: {}", tcb.tid);
        }
    }
}

pub fn current_pid() -> Option<ProcessHandle> {
    CURRENT_THREAD
        .lock()
        .as_ref()
        .and_then(|thread| thread.process.clone())
}

fn find_thread(handle: ThreadHandle) -> Option<Tcb> {
    let mut current_thread = CURRENT_THREAD.lock();
    let matches_current_thread = if let Some(thread) = current_thread.as_ref() {
        thread.tid == handle.0
    } else {
        false
    };

    if matches_current_thread {
        return Some(current_thread.take().unwrap());
    }

    if let Some(thread) = ACTIVE_THREADS
        .lock()
        .drain_filter(|thread| thread.tid == handle.0)
        .pop()
    {
        return Some(thread);
    }

    if let Some(thread) = BLOCKED_THREADS
        .lock()
        .drain_filter(|thread| thread.tid == handle.0)
        .pop()
    {
        return Some(thread);
    }

    None
}

pub(crate) fn exit_matching_threads(
    handles: &mut Vec<ThreadHandle>,
    cx: &mut ExceptionContext,
) -> Result<(), Error> {
    while let Some(handle) = handles.pop() {
        match find_thread(handle) {
            Some(thread) => {
                exit_thread(thread);
            }
            None => {
                return Err(Error::ThreadNotFound);
            }
        }
    }

    let thread = schedule_next_thread();
    restore_thread_context(cx, &thread);
    CURRENT_THREAD.lock().replace(thread);

    Ok(())
}

pub(crate) fn stack_validator(stack_type: StackType) -> Option<StackValidator> {
    match stack_type {
        StackType::KernelStack => {
            let (range_base, range_len) = crate::memory::map::stack_range();
            let range_base = range_base.try_into_logical().unwrap().into_virtual();
            Some(StackValidator {
                range_base,
                range_len,
            })
        }
        StackType::ProcessStack => CURRENT_THREAD
            .lock()
            .as_ref()
            .map(|thread| thread.stack.validator()),
    }
}

pub(crate) fn wait_for_pid_in_current_thread(cx: &mut ExceptionContext, pid: ProcessHandle) {
    let mut current_thread = CURRENT_THREAD.lock();

    let mut thread = current_thread
        .take()
        .expect("There is no current thread calling wait_for_pid!");
    assert!(!thread.is_idle_thread);

    save_thread_context(&mut thread, cx);

    thread.block_reason = Some(BlockReason::WaitForPid(pid));
    BLOCKED_THREADS.lock().push(thread);

    let thread = schedule_next_thread();
    restore_thread_context(cx, &thread);
    current_thread.replace(thread);
}
