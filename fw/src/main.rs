#![no_std]
#![no_main]

use embedded_graphics::{image::Image, pixelcolor::Rgb888, prelude::*, primitives::Rectangle};
use m1::boot_args::BootArgs;
use m1::display::Display;
use tinybmp::Bmp;

#[panic_handler]
fn panic_handler(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn kernel_main(boot_args: &BootArgs) {
    let bmp_data = include_bytes!("../ate_logo.bmp");

    let mut display = Display::new(&boot_args.boot_video);

    let logo = Bmp::<Rgb888>::from_slice(bmp_data).unwrap();
    let logo_size = logo.bounding_box().size;

    let x_pos = (display.width() - logo_size.width) / 2;
    let y_pos = (display.height() - logo_size.height) / 2;

    Image::new(&logo, Point::new(x_pos as i32, y_pos as i32))
        .draw(&mut display)
        .unwrap();

    display.flush();
}
