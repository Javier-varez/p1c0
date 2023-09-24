#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]

use p1c0 as _; // needed to link libentry (and _start)

use p1c0_kernel::{backtrace::Symbolicator, memory::address::VirtualAddress};

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    test_main();
}

#[test_case]
fn test_ksyms() {
    let ksyms = p1c0_kernel::backtrace::ksyms::symbolicator().unwrap();

    // Search some known symbol address
    let (symbol, offset) = ksyms
        .symbolicate(VirtualAddress::new_unaligned(kernel_main as *const _))
        .unwrap();
    assert_eq!(symbol, "kernel_main");
    assert_eq!(offset, 0);

    // And now for an invalid address!
    assert!(ksyms
        .symbolicate(VirtualAddress::new_unaligned(core::ptr::null()))
        .is_none());
}
