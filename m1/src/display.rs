use crate::boot_args::get_boot_args;
use core::fmt::{self, Write};

use embedded_graphics::{
    draw_target::DrawTarget,
    image::Image,
    mono_font::{ascii::FONT_5X7, MonoFont, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    text::{Baseline, Text},
};

use crate::font::FIRA_CODE_30;

const MAX_WIDTH: usize = 3024;
const MAX_HEIGHT: usize = 1964;

const RETINA_DEPTH_FLAG: usize = 1 << 16;

const ROW_MARGIN: u32 = 10;
const COL_MARGIN: u32 = 10;

// TODO(javier-varez): This should be protected by a spin mutex (and run in a critical section to
// prevent deadlocks). At this point using spin mutex causes a crash, that needs to be investigated.
static mut DISPLAY: Option<Display> = None;

// Align this to 128 to make the display update much faster. Also need to guarantee that base is
// the first element in the struct so that it is still aligned.
#[repr(C, align(128))]
pub struct Display {
    base: [u32; MAX_HEIGHT * MAX_WIDTH],
    width: u32,
    height: u32,
    stride: u32,
    hwbase: *mut u32,

    // Console members
    font: &'static MonoFont<'static>,
    current_row: u32,
    current_col: u32,
    max_rows: u32,
}

extern "C" {
    fn _memcpy128_aligned(dst: *mut u32, src: *const u32, num_bytes: usize);
}

impl Display {
    /// Initializes the display HW with the given logo to work as a console.
    pub fn init<T: ImageDrawable<Color = Rgb888>>(logo: &T) {
        let video_args = &get_boot_args().boot_video;
        let retina = (video_args.depth & RETINA_DEPTH_FLAG) != 0;
        let font = if retina { &FIRA_CODE_30 } else { &FONT_5X7 };
        let max_rows = (video_args.height as u32 - ROW_MARGIN * 2) / font.character_size.height;
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

        unsafe { DISPLAY.replace(disp) };
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
        // Calling _memcpy128_aligned makes display update way faster.
        // Safety:
        //   * self.hwbase is aligned to 128 bits
        //   * self.base is also aligned to 128 bits
        //   * size is a multiple of 128 bits
        //   * destination does not overlap with source
        unsafe {
            _memcpy128_aligned(
                self.hwbase,
                origin,
                display_size * core::mem::size_of::<u32>(),
            )
        };
    }

    fn scroll_up(&mut self) {
        let offset = (self.width * self.font.character_size.height) as usize;
        let count = (self.height * self.width) as usize - offset;
        let source = &self.base[offset] as *const u32;
        let destination = self.base.as_mut_ptr();

        // Use memcpy128 for speed. This over
        // Safety:
        //   * source is aligned to 128 bits
        //   * destination is also aligned to 128 bits
        //   * size is a multiple of 128 bits
        //   * destination is < source
        unsafe { _memcpy128_aligned(destination, source, count * core::mem::size_of::<u32>()) };

        // Clear last lines
        self.base.iter_mut().skip(count).for_each(|val| *val = 0);
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
                    self.scroll_up();
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
