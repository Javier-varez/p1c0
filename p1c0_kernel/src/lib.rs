#![cfg_attr(not(test), no_std)]
#![feature(allocator_api)]
#![feature(maybe_uninit_as_bytes)]
#![feature(maybe_uninit_slice)]
#![cfg_attr(test, feature(scoped_threads))]

pub mod adt;
pub mod arch;
pub mod boot_args;
pub mod chickens;
mod collections;
pub mod crc;
pub mod drivers;
mod font;
pub mod init;
pub mod macros;
pub mod memory;
pub mod print;
pub mod registers;
pub mod sync;
pub mod syscall;
pub mod thread;

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    drivers::display::_print(args);
    match print::_print(args) {
        Ok(_) => {}
        Err(print::Error::WriterLocked) => {
            // TODO(javier-varez): How do we push this to the user?
        }
        Err(e) => {
            panic!("Print failed with error: {:?}", e);
        }
    }
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
