#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]

use p1c0 as _;
// needed to link libentry (and _start)
use p1c0_macros::initcall;
use test_fwk::Status;

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    test_fwk::finish_with_status(Status::Fail);
}

#[initcall]
fn test_init_call() {
    test_main();
    test_fwk::finish_with_status(Status::Success);
}

#[test_case]
fn dummy_test() {}
