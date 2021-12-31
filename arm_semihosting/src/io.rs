use super::{arch::call_host, Operation, PointerArgs};
use cstr_core::CString;

use core::fmt;

#[derive(Debug)]
pub enum Error {
    FileNotFound,
    InvalidPath,
    ReadError,
    WriteError(isize),
    SeekError,
}

pub struct Readable;
pub struct Writeable;
pub struct ReadWriteable;

pub enum AccessType {
    Binary,
    Text,
}

pub struct File<MODE> {
    fd: usize,
    _pd: core::marker::PhantomData<MODE>,
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

#[repr(C)]
pub(crate) struct WriteArgs<'a> {
    fd: usize,
    buffer: &'a u8,
    length: usize,
}

impl<'a> PointerArgs for WriteArgs<'a> {}

#[repr(C)]
pub(crate) struct CloseArgs {
    fd: usize,
}

impl PointerArgs for CloseArgs {}

#[repr(C)]
pub(crate) struct SeekArgs {
    fd: usize,
    offset: usize,
}

impl PointerArgs for SeekArgs {}

fn open_with_mode(path: &str, mode: usize) -> Result<File<ReadWriteable>, Error> {
    let cpath = match CString::new(path) {
        Ok(path) => path,
        Err(_) => return Err(Error::InvalidPath),
    };

    let op = Operation::Open(OpenArgs {
        file_path: cpath.as_c_str() as *const _ as *const _,
        mode,
        length: path.len(),
    });

    let result = call_host(&op).0;

    if result == -1 {
        Err(Error::FileNotFound)
    } else {
        Ok(File {
            fd: result as usize,
            _pd: core::marker::PhantomData,
        })
    }
}

pub fn open(path: &str, access_type: AccessType) -> Result<File<Readable>, Error> {
    let binary = matches!(access_type, AccessType::Binary);
    let mode =
        ((Mode::Read as usize) << MODE_BIT_OFFSET) | ((binary as usize) << BINARY_BIT_OFFSET);

    open_with_mode(path, mode).map(|file| file.as_readonly())
}

pub fn open_read_write(path: &str, access_type: AccessType) -> Result<File<ReadWriteable>, Error> {
    let binary = matches!(access_type, AccessType::Binary);
    let mode = ((Mode::Write as usize) << MODE_BIT_OFFSET)
        | ((binary as usize) << BINARY_BIT_OFFSET)
        | (1 << READ_WRITE_BIT_OFFSET);

    open_with_mode(path, mode)
}

pub fn create(path: &str, access_type: AccessType) -> Result<File<Writeable>, Error> {
    let binary = matches!(access_type, AccessType::Binary);
    let mode =
        ((Mode::Write as usize) << MODE_BIT_OFFSET) | ((binary as usize) << BINARY_BIT_OFFSET);

    open_with_mode(path, mode).map(|file| file.as_writeonly())
}

pub fn append(path: &str, access_type: AccessType) -> Result<File<Writeable>, Error> {
    let binary = matches!(access_type, AccessType::Binary);
    let mode =
        ((Mode::Append as usize) << MODE_BIT_OFFSET) | ((binary as usize) << BINARY_BIT_OFFSET);

    open_with_mode(path, mode).map(|file| file.as_writeonly())
}

impl<MODE> File<MODE> {
    fn write_internal(&mut self, buffer: &[u8]) -> Result<(), Error> {
        let length = buffer.len();
        let op = Operation::Write(WriteArgs {
            fd: self.fd,
            buffer: &buffer[0],
            length,
        });

        let result = call_host(&op).0;

        if result != 0 {
            Err(Error::WriteError(result))
        } else {
            Ok(())
        }
    }

    fn read_internal(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
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

    pub fn seek(&mut self, byte_offset: usize) -> Result<(), Error> {
        let op = Operation::Seek(SeekArgs {
            fd: self.fd,
            offset: byte_offset,
        });

        let result = call_host(&op).0;

        if result < 0 {
            Err(Error::SeekError)
        } else {
            Ok(())
        }
    }
}

impl File<Readable> {
    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        self.read_internal(buffer)
    }
}

impl File<Writeable> {
    pub fn write(&mut self, buffer: &[u8]) -> Result<(), Error> {
        self.write_internal(buffer)
    }
}

impl File<ReadWriteable> {
    pub fn write(&mut self, buffer: &[u8]) -> Result<(), Error> {
        self.write_internal(buffer)
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        self.read_internal(buffer)
    }

    pub fn as_readonly(self) -> File<Readable> {
        let file = core::mem::ManuallyDrop::new(self);
        File {
            fd: file.fd,
            _pd: core::marker::PhantomData,
        }
    }

    pub fn as_writeonly(self) -> File<Writeable> {
        let file = core::mem::ManuallyDrop::new(self);
        File {
            fd: file.fd,
            _pd: core::marker::PhantomData,
        }
    }
}

impl<MODE> Drop for File<MODE> {
    fn drop(&mut self) {
        let op = Operation::Close(CloseArgs { fd: self.fd });
        let _result = call_host(&op);
    }
}

impl fmt::Write for File<Writeable> {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        self.write(s.as_bytes()).expect("No error writing files?");
        Ok(())
    }
}

impl fmt::Write for File<ReadWriteable> {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        self.write(s.as_bytes()).expect("No error writing files?");
        Ok(())
    }
}
