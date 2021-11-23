use crate::boot_args::BootVideoArgs;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PixelColor {
    _reserved: u8,
    blue: u8,
    green: u8,
    red: u8,
}

impl PixelColor {
    pub const BLACK: PixelColor = PixelColor {
        red: 0,
        green: 0,
        blue: 0,
        _reserved: 0,
    };

    pub const RED: PixelColor = PixelColor {
        red: 255,
        green: 0,
        blue: 0,
        _reserved: 0,
    };

    pub const GREEN: PixelColor = PixelColor {
        red: 0,
        green: 255,
        blue: 0,
        _reserved: 0,
    };

    pub const BLUE: PixelColor = PixelColor {
        red: 0,
        green: 0,
        blue: 255,
        _reserved: 0,
    };

    pub fn new(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            _reserved: 0,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Coordinate {
    pub x: u32,
    pub y: u32,
}

impl Coordinate {
    pub fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }
}

pub struct Display {
    hwbase: *mut PixelColor,
    width: u32,
    height: u32,
    stride: u32,
}

impl Display {
    pub fn new(video_args: &BootVideoArgs) -> Self {
        Self {
            hwbase: video_args.base as *mut PixelColor,
            width: video_args.width as u32,
            height: video_args.height as u32,
            stride: video_args.stride as u32 / 4,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn clear(&mut self) {
        self.fill_display(|_| PixelColor::BLACK);
    }

    pub fn fill_display<T>(&mut self, color_functor: T)
    where
        T: Fn(Coordinate) -> PixelColor,
    {
        for y in 0..self.height {
            for x in 0..self.width {
                let coordinate = Coordinate::new(x, y);
                self.draw_pixel(coordinate, color_functor(coordinate));
            }
        }
    }

    pub fn draw_pixel(&mut self, coordinate: Coordinate, color: PixelColor) {
        let pix_offset = coordinate.x + coordinate.y * self.stride;
        let ptr = self.hwbase as usize + (pix_offset * core::mem::size_of::<u32>() as u32) as usize;
        unsafe { core::ptr::write(ptr as *mut PixelColor, color) };
    }
}
