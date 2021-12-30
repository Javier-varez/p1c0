use super::{arch::call_host, Operation, PointerArgs};
use cstr_core::CString;

#[derive(Debug)]
pub enum Error {
    FileNotFound,
    InvalidPath,
    ReadError,
}

pub struct File {
    fd: usize,
}

#[repr(usize)]
pub enum Mode {
    Read = 0,
    Write = 1,
    Append = 2,
}

const BINARY_BIT_OFFSET: usize = 0;
const READ_WRITE_BIT_OFFSET: usize = 1;
const MODE_BIT_OFFSET: usize = 2;

#[repr(C)]
pub(crate) struct OpenArgs {
    file_path: *const u8,
    mode: usize,
    length: usize,
}

impl<'a> PointerArgs for OpenArgs {}

#[repr(C)]
pub(crate) struct ReadArgs<'a> {
    fd: usize,
    buffer: &'a mut u8,
    length: usize,
}

impl<'a> PointerArgs for ReadArgs<'a> {}

pub struct OpenOptions<'a> {
    path: &'a str,
    binary: bool,
    read_and_write: bool,
    mode: Mode,
}

impl<'a> OpenOptions<'a> {
    pub fn new(path: &'a str, mode: Mode) -> Self {
        Self {
            path,
            mode,
            read_and_write: false,
            binary: false,
        }
    }

    pub fn set_binary(&mut self, binary: bool) {
        self.binary = binary;
    }

    pub fn set_read_write(&mut self, read_and_write: bool) {
        self.read_and_write = read_and_write;
    }

    pub fn open(self) -> Result<File, Error> {
        let cpath = match CString::new(self.path) {
            Ok(path) => path,
            Err(_) => return Err(Error::InvalidPath),
        };

        let mode = ((self.mode as usize) << MODE_BIT_OFFSET)
            | ((self.binary as usize) << BINARY_BIT_OFFSET)
            | ((self.read_and_write as usize) << READ_WRITE_BIT_OFFSET);

        let op = Operation::Open(OpenArgs {
            file_path: cpath.as_c_str() as *const _ as *const _,
            mode,
            length: self.path.len(),
        });

        let result = call_host(&op).0;

        if result == -1 {
            Err(Error::FileNotFound)
        } else {
            Ok(File {
                fd: result as usize,
            })
        }
    }
}

impl File {
    pub fn open(path: &str) -> Result<File, Error> {
        OpenOptions::new(path, Mode::Read).open()
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let length = buffer.len();
        let op = Operation::Read(ReadArgs {
            fd: self.fd,
            buffer: &mut buffer[0],
            length,
        });

        let result = call_host(&op).0;

        if result == -1 {
            Err(Error::ReadError)
        } else {
            Ok(length - result as usize)
        }
    }
}
