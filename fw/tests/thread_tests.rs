#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]

use core::time::Duration;
use p1c0 as _;
// needed to link libentry (and _start)
use p1c0_kernel::{
    drivers::{generic_timer::get_timer, interfaces::timer::Timer},
    sync::spinlock::SpinLock,
    thread,
};

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    thread::Builder::new().name("Test").spawn(|| {
        test_main();
    });

    thread::initialize();
}

static NUM_THREADS: SpinLock<u32> = SpinLock::new(0u32);

#[test_case]
fn test_runs_single_thread() {
    *NUM_THREADS.lock() = 0;

    thread::spawn(|| {
        let mut locked_num_threads = NUM_THREADS.lock();
        *locked_num_threads += 1;
        drop(locked_num_threads);

        loop {
            cortex_a::asm::wfi();
        }
    });

    let mut retries = 0;
    const MAX_RETRIES: u32 = 10;
    loop {
        if *NUM_THREADS.lock() == 1 {
            // Done!
            break;
        }

        if retries >= MAX_RETRIES {
            panic!("Threads did not complete!");
        }
        retries += 1;

        let timer = get_timer();
        timer.delay(Duration::from_millis(10));
    }
}

#[test_case]
fn test_runs_multiple_threads() {
    *NUM_THREADS.lock() = 0;

    thread::spawn(|| {
        let mut locked_num_threads = NUM_THREADS.lock();
        *locked_num_threads += 1;
        drop(locked_num_threads);

        loop {
            cortex_a::asm::wfi();
        }
    });

    thread::spawn(|| {
        let mut locked_num_threads = NUM_THREADS.lock();
        *locked_num_threads += 1;
        drop(locked_num_threads);

        loop {
            cortex_a::asm::wfi();
        }
    });

    let mut retries = 0;
    const MAX_RETRIES: u32 = 10;
    loop {
        if *NUM_THREADS.lock() == 2 {
            // Done!
            break;
        }

        if retries >= MAX_RETRIES {
            panic!("Threads did not complete!");
        }
        retries += 1;

        let timer = get_timer();
        timer.delay(Duration::from_millis(10));
    }
}

#[test_case]
fn test_join_thread() {
    *NUM_THREADS.lock() = 0;

    let t1 = thread::spawn(|| {
        let mut locked_num_threads = NUM_THREADS.lock();
        *locked_num_threads += 1;
    });

    let t2 = thread::spawn(|| {
        let mut locked_num_threads = NUM_THREADS.lock();
        *locked_num_threads += 1;
    });

    t1.join();
    assert!(*NUM_THREADS.lock() > 0);
    t2.join();
    assert_eq!(*NUM_THREADS.lock(), 2);
}
