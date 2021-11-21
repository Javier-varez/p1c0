#![no_std]
#![no_main]
#![feature(asm)]

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { asm!("nop") };
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {
        unsafe { asm!("nop") };
    }
}
