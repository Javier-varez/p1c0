#![no_std]
#![no_main]
#![feature(asm)]

pub mod display;

use display::{Display, PixelColor};

#[repr(C)]
pub struct BootVideoArgs {
    base: *mut u8,
    display: usize,
    stride: usize,
    width: usize,
    height: usize,
    depth: usize,
}

#[repr(C)]
pub struct BootArgs {
    revision: u16,
    version: u16,
    virt_base: usize,
    phys_base: usize,
    mem_size: usize,
    top_of_kernel_data: usize,
    boot_video: BootVideoArgs,
    machine_type: u32,
    device_tree: *mut u8,
    device_tree_size: usize,
    cmdline: [u8; 608],
    boot_flags: u64,
    mem_size_actual: u64,
}

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { asm!("nop") };
    }
}

#[no_mangle]
pub extern "C" fn kernel_main(boot_args: &BootArgs) -> ! {
    let mut display = Display::new(&boot_args.boot_video);
    display.clear();

    let height = display.height();

    display.fill_display(|coordinate| {
        if coordinate.y > (height / 3) * 2 {
            PixelColor::BLUE
        } else if coordinate.y < (height / 3) {
            PixelColor::RED
        } else {
            PixelColor::GREEN
        }
    });

    loop {
        unsafe { asm!("nop") };
    }
}
