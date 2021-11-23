use crate::boot_args::BootVideoArgs;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;

const MAX_WIDTH: usize = 3024;
const MAX_HEIGHT: usize = 1964;

pub struct Display {
    width: u32,
    height: u32,
    stride: u32,
    hwbase: *mut u32,
    base: [u32; MAX_HEIGHT * MAX_WIDTH],
}

impl Display {
    pub fn new(video_args: &BootVideoArgs) -> Self {
        Self {
            hwbase: video_args.base as *mut u32,
            width: video_args.width as u32,
            height: video_args.height as u32,
            stride: video_args.stride as u32 / 4,
            base: [0; MAX_HEIGHT * MAX_WIDTH],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn flush(&mut self) {
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
