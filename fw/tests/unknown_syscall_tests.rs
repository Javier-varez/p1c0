#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner_should_panic)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]

use p1c0 as _; // needed to link libentry (and _start)
use p1c0_kernel::syscall::Syscall;

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler_should_panic(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    test_main();
}

#[test_case]
fn test_unknown_syscall() {
    unsafe {
        core::arch::asm!("svc #0xFFFF");
    }
}
