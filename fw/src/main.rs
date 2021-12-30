#![no_std]
#![no_main]
#![feature(default_alloc_error_handler)]

use m1::println;

use embedded_graphics::pixelcolor::Rgb888;
use m1::boot_args::{get_boot_args, BootArgs};
use m1::display::Display;
use tinybmp::Bmp;

const ATE_LOGO_DATA: &[u8] = include_bytes!("../ate_logo.bmp");

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    println!("Panicked with message: {:?}", panic_info);

    #[cfg(feature = "emulator")]
    arm_semihosting::exit(1);

    #[cfg(not(feature = "emulator"))]
    loop {}
}

fn print_boot_args(boot_args: &BootArgs) {
    println!("===== BOOT ARGS =====");
    println!("Revision:           {}", boot_args.revision);
    println!("Version:            {}", boot_args.version);
    println!("Virtual base:       0x{:x}", boot_args.virt_base);
    println!("Physical base:      0x{:x}", boot_args.phys_base);
    println!("Mem size:           0x{:x}", boot_args.mem_size);
    println!("Top of kernel data: 0x{:x}", boot_args.top_of_kernel_data);
    println!("Video base:         {:?}", boot_args.boot_video.base);
    println!("Video num displays: {}", boot_args.boot_video.display);
    println!("Video stride:       {}", boot_args.boot_video.stride);
    println!("Video width:        {}", boot_args.boot_video.width);
    println!("Video height:       {}", boot_args.boot_video.height);
    println!("Video depth:        0x{:x}", boot_args.boot_video.depth);
    println!("Machine type:       {}", boot_args.machine_type);
    println!("Device tree:        {:?}", boot_args.device_tree);
    println!("Device tree size:   0x{:x}", boot_args.device_tree_size);
    println!("Boot flags:         {}", boot_args.boot_flags);
    println!("Mem size actual:    0x{:x}", boot_args.mem_size_actual);
}

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    let logo = Bmp::<Rgb888>::from_slice(ATE_LOGO_DATA).unwrap();
    Display::init(&logo);

    println!("p1c0 running on Apple M1 Pro");
    println!("Exception level: {:?}", m1::arch::get_exception_level());
    println!();

    let boot_args = get_boot_args();
    print_boot_args(boot_args);

    let addr = 0x00007FFFFFFFFFFF as *const u64;
    println!("let's cause a page fault!");
    let val = unsafe { *addr };
    println!("Hah, value is {}", val);

    #[cfg(feature = "emulator")]
    arm_semihosting::exit(0);

    #[cfg(not(feature = "emulator"))]
    loop {}
}
