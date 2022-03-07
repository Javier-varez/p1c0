#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]
#![feature(assert_matches)]

use p1c0 as _; // needed to link libentry (and _start)
use p1c0_kernel::drivers::aic::{Aic, IrqType};

use core::assert_matches::assert_matches;

use cortex_a::registers::DAIF;
use tock_registers::interfaces::Writeable;

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    // Mask interrupts
    DAIF.write(DAIF::I::Masked);

    test_main();
}

#[test_case]
fn test_probe_aic() {
    let _aic = Aic::probe("/arm-io/aic").unwrap();
}

#[test_case]
fn test_generate_sw_interrupt() {
    let mut aic = Aic::probe("/arm-io/aic").unwrap();

    assert_matches!(aic.get_current_irq(), None);

    aic.unmask_interrupt(0).unwrap();
    aic.set_interrupt(0).unwrap();

    assert_matches!(aic.get_current_irq(), Some((0, 0, IrqType::HW)));
    assert_matches!(aic.get_current_irq(), None);

    aic.unmask_interrupt(1).unwrap();
    aic.set_interrupt(1).unwrap();

    assert_matches!(aic.get_current_irq(), Some((0, 1, IrqType::HW)));
    assert_matches!(aic.get_current_irq(), None);
}
