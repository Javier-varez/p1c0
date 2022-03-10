extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use cortex_a::{
    asm::{barrier, wfi},
    registers::{ELR_EL1, SPSR_EL1, SP_EL0},
};
use heapless::String;
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::{
    arch::exceptions::ExceptionContext,
    collections::{
        intrusive_list::{IntrusiveItem, IntrusiveList},
        OwnedMutPtr,
    },
    drivers::interfaces::{timer::Timer, Ticks},
    sync::spinlock::SpinLock,
};

use crate::drivers::generic_timer::get_timer;
use core::ops::Add;
use core::time::Duration;
use core::{
    arch::asm,
    sync::atomic::{AtomicU64, Ordering},
};

struct Stack(Vec<u64>);

impl Stack {
    fn new(size: usize) -> Self {
        let mut stack: Vec<u64> = Vec::with_capacity(size);
        #[allow(clippy::uninit_vec)]
        unsafe {
            stack.set_len(size)
        };
        Self(stack)
    }

    fn top(&self) -> u64 {
        &self.0[self.0.len() - 1] as *const _ as u64
    }
}

enum BlockReason {
    Sleep(Ticks),
}

pub struct ThreadControlBlock {
    tid: u64,
    name: String<32>,
    entry: Option<Box<dyn FnOnce()>>,
    stack: Stack,

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

static NUM_THREADS: AtomicU64 = AtomicU64::new(0);

extern "C" fn thread_start(thread_control_block: &mut ThreadControlBlock) {
    match thread_control_block.entry.take() {
        Some(closure) => closure(),
        _ => panic!("Expected to find Entry::Start"),
    };
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

    pub fn spawn<F>(self, thread: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let thread_wrapper = Box::new(move || {
            // TODO(javier-varez): Thread initialization here
            thread();
            // TODO(javier-varez): Thread cleanup here
            // At this point we should destroy the thread and make sure its execution stops here.
            // Otherwise we might return into the void triggering an exception.
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

        let mut tcb = OwnedMutPtr::new_from_box(Box::new(IntrusiveItem::new(ThreadControlBlock {
            tid: NUM_THREADS.fetch_add(1, Ordering::Relaxed),
            name,
            entry: Some(thread_wrapper),
            stack,
            block_reason: None,
            regs,
            elr: elr as u64,
            spsr: spsr.get(),
            stack_ptr,
        })));
        tcb.regs[0] = (&mut **tcb) as *mut ThreadControlBlock as u64;

        ACTIVE_THREADS.lock().push(tcb);
    }
}

pub fn spawn<F>(thread: F)
where
    F: FnOnce() + Send + 'static,
{
    Builder::new().spawn(thread);
}

pub fn initialize() -> ! {
    let mut current_thread = CURRENT_THREAD.lock();
    assert!(current_thread.is_none());

    // Spawn idle thread
    Builder::new().name("Idle").stack_size(128).spawn(|| loop {
        wfi();
    });

    // Let's take the first element in the thread list and run that
    let thread = ACTIVE_THREADS.lock().pop().expect("No threads found!");
    current_thread.replace(thread);

    let tcb = current_thread.as_mut().unwrap();

    // Setting the EL0 thread stack pointer. This is used instead of the EL1 SP.
    SP_EL0.set(tcb.stack.top());
    ELR_EL1.set(thread_start as usize as u64);
    SPSR_EL1.modify(SPSR_EL1::M::EL1t);

    // Taking a static reference to the TCB is actually safe because it is used on thread entry and
    // only in the context of the thread. As long as the thread is alive it should be safe to keep
    // it. Note that the link in the list cannot be mutated via this reference
    let tcb_raw = (&mut ***tcb) as *mut ThreadControlBlock;
    drop(current_thread);

    // Jump to thread immediately
    unsafe {
        barrier::dsb(barrier::SY);
        asm!(
        "mov x0, {}",
        "eret",
        in(reg) tcb_raw);
    }
    unreachable!();
}

fn save_thread_context(thread: &mut Tcb, cx: &mut ExceptionContext) {
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

fn schedule_next_thread() -> Tcb {
    wake_asleep_threads();

    // This is the actual round-robin scheduling algo... For now it works, but it is obviously not
    // optimal
    ACTIVE_THREADS.lock().pop().unwrap()
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

    // Store the thread in the list again
    ACTIVE_THREADS.lock().push(thread);

    let thread = schedule_next_thread();
    restore_thread_context(cx, &thread);

    current_thread.replace(thread);
}

pub fn sleep_current_thread(cx: &mut ExceptionContext, duration: Duration) {
    let mut current_thread = CURRENT_THREAD.lock();

    let mut thread = current_thread
        .take()
        .expect("There is no current thread calling sleep!");

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

pub fn print_thread_info() {
    let current_thread = CURRENT_THREAD.lock();
    let threads = ACTIVE_THREADS.lock();

    crate::println!("Thread information:");
    if let Some(tcb) = &*current_thread {
        if let Some(name) = tcb.name() {
            crate::println!("\tCurrent thread: {}, tid: {}", name, tcb.tid);
        } else {
            crate::println!("\tCurrent thread tid: {}", tcb.tid);
        }
    }

    for tcb in threads.iter() {
        if let Some(name) = tcb.name() {
            crate::println!("\tThread: {}, tid: {}", name, tcb.tid);
        } else {
            crate::println!("\tAnonymous thread, tid: {}", tcb.tid);
        }
    }
    crate::println!();
}
