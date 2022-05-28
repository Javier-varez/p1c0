#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]

use p1c0 as _; // needed to link libentry (and _start)

use core::sync::atomic::{AtomicBool, Ordering};

use p1c0_macros::initcall;

use test_fwk::Status;

static HIGH_PRIO_RUN: AtomicBool = AtomicBool::new(false);
static MEDIUM_PRIO_RUN: AtomicBool = AtomicBool::new(false);

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    test_fwk::finish_with_status(Status::Fail);
}

#[initcall(priority = 4)]
fn test_initcall_with_high_prio() {
    HIGH_PRIO_RUN.store(true, Ordering::Relaxed);
}

#[initcall(priority = 3)]
fn test_initcall_with_medium_prio() {
    MEDIUM_PRIO_RUN.store(true, Ordering::Relaxed);
}

#[initcall]
fn test_initcall_with_normal_prio() {
    test_main();
}

#[test_case]
fn check_high_priority_did_run() {
    assert!(HIGH_PRIO_RUN.load(Ordering::Relaxed));
}

#[test_case]
fn check_medium_priority_did_run() {
    assert!(MEDIUM_PRIO_RUN.load(Ordering::Relaxed));
}
