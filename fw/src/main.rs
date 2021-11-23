#![no_std]
#![no_main]

use m1::boot_args::BootArgs;
use m1::display::{Display, PixelColor};

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn kernel_main(boot_args: &BootArgs) {
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
}
