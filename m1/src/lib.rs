#![cfg_attr(not(test), no_std)]
#![feature(allocator_api)]

pub mod adt;
pub mod arch;
pub mod boot_args;
pub mod chickens;
mod collections;
pub mod display;
pub mod font;
pub mod init;
pub mod macros;
pub mod registers;
pub mod spi;
pub mod uart;
pub mod wdt;

const ADT_VIRT_BASE: usize = 0xFFFF000000000000;
const KERNEL_LOGICAL_BASE: usize = 0xFFFF020000000000;

pub fn pa_to_kla<T>(pa: *const T) -> *const T {
    if cfg!(test) {
        pa
    } else {
        (pa as usize + KERNEL_LOGICAL_BASE) as *const _
    }
}

pub fn pa_to_kla_mut<T>(pa: *mut T) -> *mut T {
    if cfg!(test) {
        pa
    } else {
        (pa as usize + KERNEL_LOGICAL_BASE) as *mut _
    }
}

pub fn kla_to_pa<T>(kla: *const T) -> *const T {
    if cfg!(test) {
        kla
    } else {
        (kla as usize - KERNEL_LOGICAL_BASE) as *const _
    }
}

pub fn kla_to_pa_mut<T>(kla: *mut T) -> *mut T {
    if cfg!(test) {
        kla
    } else {
        (kla as usize - KERNEL_LOGICAL_BASE) as *mut _
    }
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    display::_print(args);
    uart::_print(args);
}

/// Prints to the host through the display console interface
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::_print(format_args!($($arg)*));
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
