#![no_std]
#![no_main]
#![feature(default_alloc_error_handler)]

extern crate alloc;
use alloc::vec::Vec;

use embedded_graphics::pixelcolor::Rgb888;
use tinybmp::Bmp;

use p1c0::print_boot_args;

use p1c0_kernel::{
    adt::{get_adt, Adt},
    arch::get_exception_level,
    boot_args::get_boot_args,
    drivers::{display::Display, spi::Spi},
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
    println!("Exception level: {:?}", get_exception_level());
    println!();

    let boot_args = get_boot_args();
    print_boot_args(boot_args);

    #[cfg(feature = "emulator")]
    print_semihosting_caps();

    let adt = get_adt().expect("Valid ADT");
    print_compatible(&adt);
    print_uart_reg(&adt);

    let mut spi3 = unsafe { Spi::new("/arm-io/spi3").unwrap() };

    // 1 byte packing
    let buffer = [1u8, 2, 3, 4, 5];
    let mut recv = [31u8; 4];
    spi3.transact(&buffer, &mut recv).unwrap();

    // 2 byte packing
    let buffer = [1u8, 2, 3, 4, 5, 6];
    let mut recv = [31u8; 12];
    spi3.transact(&buffer, &mut recv).unwrap();

    // 4 byte packing
    let buffer = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let mut recv = [31u8; 16];
    spi3.transact(&buffer, &mut recv).unwrap();

    // only rx
    let buffer = [];
    let mut recv = [31u8; 16];
    spi3.transact(&buffer, &mut recv).unwrap();

    // only tx
    let buffer = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let mut recv = [];
    spi3.transact(&buffer, &mut recv).unwrap();

    // Trigger a random interrupt
    let mut aic = p1c0_kernel::drivers::aic::Aic::probe("/arm-io/aic").unwrap();

    unsafe {
        p1c0_kernel::drivers::aic::AIC.replace(aic);

        if let Some(aic) = &mut p1c0_kernel::drivers::aic::AIC {
            aic.unmask_interrupt(15).unwrap();
            aic.set_interrupt(15).unwrap();
        }
    }
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
