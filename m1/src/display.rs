use crate::boot_args::BootVideoArgs;
use core::fmt::{self, Write};

use embedded_graphics::{
    draw_target::DrawTarget,
    image::Image,
    mono_font::{
        ascii::{FONT_10X20, FONT_5X7},
        MonoFont, MonoTextStyle,
    },
    pixelcolor::Rgb888,
    prelude::*,
    text::{Baseline, Text},
};

const MAX_WIDTH: usize = 3024;
const MAX_HEIGHT: usize = 1964;

const RETINA_DEPTH_FLAG: usize = 1 << 16;

const ROW_MARGIN: u32 = 10;
const COL_MARGIN: u32 = 10;

// TODO(javier-varez): This should be protected by a spin mutex (and run in a critical section to
// prevent deadlocks). At this point using spin mutex causes a crash, that needs to be investigated.
static mut DISPLAY: Option<Display> = None;

pub struct Display {
    width: u32,
    height: u32,
    stride: u32,
    hwbase: *mut u32,
    base: [u32; MAX_HEIGHT * MAX_WIDTH],

    // Console members
    font: &'static MonoFont<'static>,
    current_row: u32,
    current_col: u32,
    max_rows: u32,
}

impl Display {
    pub unsafe fn init<T: ImageDrawable<Color = Rgb888>>(video_args: &BootVideoArgs, logo: &T) {
        let retina = (video_args.depth & RETINA_DEPTH_FLAG) != 0;
        let font = if retina { &FONT_10X20 } else { &FONT_5X7 };
        let max_rows = (video_args.height as u32 / font.character_size.height) - ROW_MARGIN * 2;
        let mut disp = Self {
            hwbase: video_args.base as *mut u32,
            width: video_args.width as u32,
            height: video_args.height as u32,
            stride: video_args.stride as u32 / 4,
            base: [0; MAX_HEIGHT * MAX_WIDTH],
            font,
            current_row: 0,
            current_col: 0,
            max_rows,
        };

        disp.draw_logo(logo);
        disp.flush();

        DISPLAY.replace(disp);
    }

    fn draw_logo<T: ImageDrawable<Color = Rgb888>>(&mut self, logo: &T) {
        let logo_size = logo.bounding_box().size;

        let x_pos = (self.width - logo_size.width) / 2;
        let y_pos = (self.height - logo_size.height) / 2;

        Image::new(logo, Point::new(x_pos as i32, y_pos as i32))
            .draw(self)
            .ok();
    }

    fn flush(&mut self) {
        let display_size = (self.stride * self.height) as usize;
        let origin = self.base.as_ptr();
        unsafe { core::ptr::copy_nonoverlapping(origin, self.hwbase, display_size) };
    }
}

impl DrawTarget for Display {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            let Point { x, y } = coord;

            // Ignore whatever falls outside of the display
            if x >= self.width as i32 || x < 0 || y >= self.height as i32 || y < 0 {
                continue;
            }

            // Calculate the index in the framebuffer.
            let pix_offset = (x + y * self.stride as i32) as usize;
            let color =
                (color.r() as u32) << 22 | (color.g() as u32) << 12 | (color.b() as u32) << 2;
            self.base[pix_offset] = color;
        }

        Ok(())
    }
}

impl OriginDimensions for Display {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl fmt::Write for Display {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        let splits = s.split_inclusive("\n");

        let style = MonoTextStyle::new(self.font, Rgb888::WHITE);
        for sub in splits {
            let x_pos = COL_MARGIN + self.current_col * self.font.character_size.width;
            let y_pos = ROW_MARGIN + self.current_row * self.font.character_size.height;
            Text::with_baseline(
                sub,
                Point::new(x_pos as i32, y_pos as i32),
                style,
                Baseline::Top,
            )
            .draw(self)
            .expect("draw is infallible");

            if sub.ends_with('\n') {
                self.flush();
                self.current_row += 1;
                self.current_col = 0;
                if self.current_row >= self.max_rows {
                    // TODO(javier-varez): Implement scrolling here
                    self.current_row = self.max_rows - 1;
                }
            } else {
                self.current_col += sub.len() as u32;
            }
        }

        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    if let Some(display) = unsafe { DISPLAY.as_mut() } {
        display.write_fmt(args).expect("Printing to display failed");
    }
}

/// Prints to the host through the display console interface
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::display::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the display console interface, appending a newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
  concat!($fmt, "\n"), $($arg)*));
}
