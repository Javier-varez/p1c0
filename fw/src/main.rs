#![no_std]
#![no_main]
#![feature(default_alloc_error_handler)]

extern crate alloc;
use alloc::vec::Vec;

use embedded_graphics::pixelcolor::Rgb888;
use tinybmp::Bmp;

use p1c0::print_boot_args;

use m1::{
    adt::{get_adt, Adt},
    boot_args::get_boot_args,
    display::Display,
    println,
};

#[cfg(feature = "emulator")]
use p1c0::print_semihosting_caps;

const ATE_LOGO_DATA: &[u8] = include_bytes!("../ate_logo.bmp");

fn print_compatible(adt: &Adt) {
    let node = adt.find_node("/").expect("There is a root node");
    let compat = node
        .find_property("compatible")
        .expect("There is a compatible prop");
    let compatibles: Vec<_> = compat.str_list_value().collect();
    println!("ADT Compatible: {:?}", compatibles);
    println!()
}

fn print_uart_reg(adt: &Adt) {
    let reg = adt
        .find_node("/arm-io/uart0")
        .and_then(|uart_node| uart_node.find_property("reg"))
        .expect("The UART0 has a \"reg\" property");
    println!("Uart reg: 0x{:x}", reg.usize_value().unwrap());
    println!()
}

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

    let adt = get_adt().expect("Valid ADT");
    print_compatible(&adt);
    print_uart_reg(&adt);

    let addr = 0x00007FFFFFFFFFFF as *const u64;
    println!("let's cause a page fault!");
    let val = unsafe { *addr };
    println!("Hah, value is {}", val);
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    kernel_entry();

    #[cfg(feature = "emulator")]
    arm_semihosting::exit(0);

    #[cfg(not(feature = "emulator"))]
    loop {}
}

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    println!("Panicked with message: {:?}", panic_info);

    #[cfg(feature = "emulator")]
    arm_semihosting::exit(1);

    #[cfg(not(feature = "emulator"))]
    loop {}
}
