#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(assert_matches)]

use p1c0 as _; // needed to link libentry (and _start)

use core::assert_matches::assert_matches;

use p1c0_kernel::drivers::interfaces::interrupt_controller::{may_do_with_irq_controller, IrqType};

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    test_main();
}

#[test_case]
fn test_probe_aic() {
    // Aic should have been probed. Try to obtain a reference and check we get a valid instance
    let mut body_runs = false;
    assert!(may_do_with_irq_controller(|_controller| {
        body_runs = true;
    }));
    assert!(body_runs);
}

#[test_case]
fn test_generate_sw_interrupt() {
    assert!(may_do_with_irq_controller(|controller| {
        assert_matches!(controller.get_current_irq(), None);

        controller.unmask_interrupt(0).unwrap();
        controller.set_interrupt(0).unwrap();

        assert_matches!(controller.get_current_irq(), Some((0, 0, IrqType::HW)));
        assert_matches!(controller.get_current_irq(), None);

        controller.unmask_interrupt(1).unwrap();
        controller.set_interrupt(1).unwrap();

        assert_matches!(controller.get_current_irq(), Some((0, 1, IrqType::HW)));
        assert_matches!(controller.get_current_irq(), None);
    }));
}
