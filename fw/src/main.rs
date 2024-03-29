#![no_std]
#![no_main]

use p1c0::print_boot_args;

#[cfg(feature = "emulator")]
use p1c0::print_semihosting_caps;

use core::sync::atomic::{AtomicBool, Ordering};

use p1c0_kernel::{
    arch::get_exception_level,
    boot_args::get_boot_args,
    drivers::display::Display,
    prelude::*,
    syscall::Syscall,
    thread::{self, print_thread_info},
};

#[cfg(not(feature = "emulator"))]
use p1c0_kernel::drivers::{gpio::GpioBank, hid::HidDev, spi::Spi};

use aarch64_cpu::registers::DAIF;
use embedded_graphics::pixelcolor::Rgb888;
use tinybmp::Bmp;
use tock_registers::interfaces::Writeable;

const ATE_LOGO_DATA: &[u8] = include_bytes!("../ate_logo.bmp");

fn kernel_entry() {
    let logo = Bmp::<Rgb888>::from_slice(ATE_LOGO_DATA).unwrap();
    Display::init(&logo);

    log_debug!("p1c0 running on Apple M1 Pro");
    log_debug!("Exception level: {:?}", get_exception_level());

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

            log_info!("Count {}", count);
            count += 1;
            Syscall::sleep_us(1_000_000);
        }
    });

    thread::spawn(move || loop {
        log_info!("Second thread");
        Syscall::sleep_us(750_000);
    });

    #[cfg(not(feature = "emulator"))]
    thread::Builder::new().name("HID").spawn(move || {
        if let Ok(spi3) = unsafe { Spi::new("/arm-io/spi3") } {
            if let Ok(gpio0_bank) = unsafe { GpioBank::new("/arm-io/gpio0") } {
                let nub_gpio0_bank = unsafe { GpioBank::new("/arm-io/nub-gpio0").unwrap() };

                let mut hid_dev = unsafe {
                    HidDev::new("/arm-io/spi3/ipd", spi3, &gpio0_bank, &nub_gpio0_bank).unwrap()
                };
                hid_dev.power_on();
                loop {
                    // Handle HID events
                    hid_dev.process();
                }
            }
        }
    });

    thread::spawn(move || {
        let filename = "/bin/virtio";
        let mut file = p1c0_kernel::filesystem::VirtualFileSystem::open(
            filename,
            p1c0_kernel::filesystem::OpenMode::Read,
        )
        .unwrap();

        let mut elf_data = vec![0; file.size];
        p1c0_kernel::filesystem::VirtualFileSystem::read(&mut file, &mut elf_data[..]).unwrap();
        p1c0_kernel::filesystem::VirtualFileSystem::close(file);
        let builder =
            p1c0_kernel::process::Builder::new_from_elf_data(filename, elf_data, 0).unwrap();
        builder.start().unwrap();
    });

    thread::initialize();
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    kernel_entry();

    #[cfg(feature = "emulator")]
    arm_semihosting::exit(0);

    #[cfg(not(feature = "emulator"))]
    loop {
        aarch64_cpu::asm::wfi();
    }
}

fn finish() -> ! {
    #[cfg(feature = "emulator")]
    arm_semihosting::exit(1);

    #[cfg(not(feature = "emulator"))]
    loop {
        aarch64_cpu::asm::wfi();
    }
}

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    // Mask interrupts.
    DAIF.write(DAIF::D::Masked + DAIF::A::Masked + DAIF::I::Masked + DAIF::F::Masked);

    static ALREADY_PANICKED: AtomicBool = AtomicBool::new(false);
    if ALREADY_PANICKED.load(Ordering::Relaxed) {
        log_error!(
            "Panicked while panicking! Reduced panic info: {:?}",
            panic_info
        );
        unsafe {
            print::force_flush();
        }
        finish();
    }
    ALREADY_PANICKED.store(true, Ordering::Relaxed);

    unsafe {
        print::force_flush();
    }

    log_error!("Panicked with message: {:?}", panic_info);
    if let Some(bt) = p1c0_kernel::backtrace::kernel_backtracer() {
        log_error!("{}", bt);
    }

    unsafe {
        print::force_flush();
    }
    finish();
}
