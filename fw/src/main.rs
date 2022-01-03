#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner_should_panic)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]

use m1::println;

use embedded_graphics::pixelcolor::Rgb888;
use m1::boot_args::get_boot_args;
use m1::display::Display;
use tinybmp::Bmp;

use p1c0::print_boot_args;

#[cfg(feature = "emulator")]
use p1c0::print_semihosting_caps;

const ATE_LOGO_DATA: &[u8] = include_bytes!("../ate_logo.bmp");

fn kernel_entry() {
    let logo = Bmp::<Rgb888>::from_slice(ATE_LOGO_DATA).unwrap();
    Display::init(&logo);

    println!("p1c0 running on Apple M1 Pro");
    println!("Exception level: {:?}", m1::arch::get_exception_level());
    println!();

    let boot_args = get_boot_args();
    print_boot_args(boot_args);

    #[cfg(feature = "emulator")]
    print_semihosting_caps();

    let addr = 0x00007FFFFFFFFFFF as *const u64;
    println!("let's cause a page fault!");
    let val = unsafe { *addr };
    println!("Hah, value is {}", val);
}

#[no_mangle]
#[cfg(not(test))]
pub extern "C" fn kernel_main() -> ! {
    kernel_entry();

    #[cfg(feature = "emulator")]
    arm_semihosting::exit(0);

    #[cfg(not(feature = "emulator"))]
    loop {}
}

#[panic_handler]
#[cfg(not(test))]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    println!("Panicked with message: {:?}", panic_info);

    #[cfg(feature = "emulator")]
    arm_semihosting::exit(1);

    #[cfg(not(feature = "emulator"))]
    loop {}
}

#[panic_handler]
#[cfg(test)]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler_should_panic(panic_info);
}

#[no_mangle]
#[cfg(test)]
pub extern "C" fn kernel_main() {
    #[cfg(test)]
    test_main();
}

#[test_case]
fn test_kernel_entry() {
    // Currently this is expected to panic
    kernel_entry();
}
