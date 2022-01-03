use super::{arch::call_host_unchecked, Operation, PointerArgs};
use core::fmt::Write;

#[cfg(feature = "alloc")]
use cstr_core::CString;

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::string::String;

#[repr(C)]
pub(crate) struct WritecArgs {
    c: u8,
}

impl PointerArgs for WritecArgs {}

#[cfg(feature = "alloc")]
#[repr(C)]
pub(crate) struct Write0Args {
    string: CString,
}

#[cfg(feature = "alloc")]
impl Write0Args {
    pub fn get_args(&self) -> usize {
        self.string.as_ptr() as usize
    }
}

pub fn write_char(c: u8) {
    let mut op = Operation::Writec(WritecArgs { c });
    unsafe { call_host_unchecked(&mut op) };
}

#[cfg(feature = "alloc")]
pub fn write_line(s: &str) {
    let mut string = String::from(s);
    string.push('\n');
    let string = CString::new(string).unwrap();
    let mut op = Operation::Write0(Write0Args { string });
    unsafe { call_host_unchecked(&mut op) };
}

#[cfg(not(feature = "alloc"))]
pub fn write_line(s: &str) {
    s.chars().for_each(|c| write_char(c as u8));
    write_char(b'\n');
}

#[cfg(feature = "alloc")]
pub fn write_str(s: &str) {
    let string = CString::new(s).unwrap();
    let mut op = Operation::Write0(Write0Args { string });
    unsafe { call_host_unchecked(&mut op) };
}

#[cfg(not(feature = "alloc"))]
pub fn write_str(s: &str) {
    s.chars().for_each(|c| write_char(c as u8));
}

pub fn read_char() -> u8 {
    let mut op = Operation::Readc;
    unsafe { call_host_unchecked(&mut op) as u8 }
}

#[cfg(feature = "alloc")]
pub fn read_line() -> String {
    let mut string = String::new();

    loop {
        let c = read_char();
        if c == b'\n' || c == b'\r' {
            string.push('\n');
            break string;
        }
        string.push(c as char);
    }
}

struct Serial {}

impl Write for Serial {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_str(s);
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print_args(args: ::core::fmt::Arguments) {
    Serial {}
        .write_fmt(args)
        .expect("Printing to semihosting console failed");
}

/// Prints to the host through the semihosting api
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::serial::_print_args(format_args!($($arg)*));
    };
}

/// Prints to the host through the semihosting api, appending a newline
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
  concat!($fmt, "\n"), $($arg)*));
}
