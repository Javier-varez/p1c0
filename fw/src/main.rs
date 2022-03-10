#![no_std]
#![no_main]
#![feature(default_alloc_error_handler)]

use embedded_graphics::pixelcolor::Rgb888;
use tinybmp::Bmp;

use p1c0::print_boot_args;

use p1c0_kernel::{
    arch::get_exception_level,
    boot_args::get_boot_args,
    drivers::{display::Display, gpio::GpioBank, hid::HidDev, spi::Spi, wdt},
    println,
    syscall::Syscall,
    thread::{self, print_thread_info},
};

use cortex_a::registers::DAIF;
use tock_registers::interfaces::Writeable;

#[cfg(feature = "emulator")]
use p1c0::print_semihosting_caps;

const ATE_LOGO_DATA: &[u8] = include_bytes!("../ate_logo.bmp");

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

    thread::spawn(move || {
        print_thread_info();

        let mut count = 0;
        loop {
            if count > 10 {
                Syscall::reboot();
            }

            println!("Count {}", count);
            count += 1;
            Syscall::sleep_us(1_000_000);
        }
    });

    thread::spawn(move || loop {
        println!("Second thread");
        Syscall::sleep_us(750_000);
    });

    #[cfg(not(feature = "emulator"))]
    thread::Builder::new().name("HID").spawn(move || {
        let spi3 = unsafe { Spi::new("/arm-io/spi3").unwrap() };
        let gpio0_bank = unsafe { GpioBank::new("/arm-io/gpio0").unwrap() };
        let nub_gpio0_bank = unsafe { GpioBank::new("/arm-io/nub-gpio0").unwrap() };

        let mut hid_dev =
            unsafe { HidDev::new("/arm-io/spi3/ipd", spi3, &gpio0_bank, &nub_gpio0_bank).unwrap() };
        hid_dev.power_on();
        loop {
            // Handle HID events
            hid_dev.process();
        }
    });

    thread::Builder::new().name("WDT").spawn(move || loop {
        wdt::service();

        Syscall::sleep_us(1_000_000);
    });

    thread::initialize();
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
    // Mask interrupts.
    DAIF.write(DAIF::D::Masked + DAIF::A::Masked + DAIF::I::Masked + DAIF::F::Masked);
    println!("Panicked with message: {:?}", panic_info);

    #[cfg(feature = "emulator")]
    arm_semihosting::exit(1);

    #[cfg(not(feature = "emulator"))]
    loop {}
}
