use super::{arch::call_host_unchecked, Operation, PointerArgs};

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
    let op = Operation::Writec(WritecArgs { c });
    unsafe { call_host_unchecked(&op) };
}

#[cfg(feature = "alloc")]
pub fn write_line(s: &str) {
    let mut string = String::from(s);
    string.push('\n');
    let string = CString::new(string).unwrap();
    let op = Operation::Write0(Write0Args { string });
    unsafe { call_host_unchecked(&op) };
}

#[cfg(not(feature = "alloc"))]
pub fn write_line(s: &str) {
    s.chars().for_each(|c| write_char(c as u8));
    write_char(b'\n');
}

pub fn read_char() -> u8 {
    let op = Operation::Readc;
    let byte = unsafe { call_host_unchecked(&op) } as u8;
    byte
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
