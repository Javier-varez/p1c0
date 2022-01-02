use super::{call_host, Errno, Operation, PointerArgs};

use cstr_core::CString;

use core::fmt;

#[derive(Debug)]
pub enum Error {
    InvalidPath,
    EndOfFile,
    WriteError(usize),
    Errno(Errno),
}

impl From<Errno> for Error {
    fn from(errno: Errno) -> Self {
        Error::Errno(errno)
    }
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

#[repr(C)]
pub(crate) struct FlenArgs {
    fd: usize,
}

impl PointerArgs for FlenArgs {}

#[repr(C)]
pub(crate) struct RemoveArgs {
    file_path: *const u8,
    length: usize,
}

impl PointerArgs for RemoveArgs {}

#[repr(C)]
pub(crate) struct RenameArgs {
    file_path: *const u8,
    length: usize,
    new_file_path: *const u8,
    new_length: usize,
}

impl PointerArgs for RenameArgs {}

fn open_with_mode(path: &str, mode: usize) -> Result<File<ReadWriteable>, Error> {
    let cpath = match CString::new(path) {
        Ok(path) => path,
        Err(_) => return Err(Error::InvalidPath),
    };

    let mut op = Operation::Open(OpenArgs {
        file_path: cpath.as_c_str() as *const _ as *const _,
        mode,
        length: path.len(),
    });

    let result = call_host(&mut op)?;

    Ok(File {
        fd: result,
        _pd: core::marker::PhantomData,
    })
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

pub fn remove(path: &str) -> Result<(), Error> {
    let cpath = match CString::new(path) {
        Ok(path) => path,
        Err(_) => return Err(Error::InvalidPath),
    };

    let mut op = Operation::Remove(RemoveArgs {
        file_path: cpath.as_c_str() as *const _ as *const _,
        length: path.len(),
    });

    call_host(&mut op)?;
    Ok(())
}

pub fn rename(path: &str, new_path: &str) -> Result<(), Error> {
    let cpath = match CString::new(path) {
        Ok(path) => path,
        Err(_) => return Err(Error::InvalidPath),
    };

    let new_cpath = match CString::new(new_path) {
        Ok(path) => path,
        Err(_) => return Err(Error::InvalidPath),
    };

    let mut op = Operation::Rename(RenameArgs {
        file_path: cpath.as_c_str() as *const _ as *const _,
        length: path.len(),
        new_file_path: new_cpath.as_c_str() as *const _ as *const _,
        new_length: new_path.len(),
    });

    call_host(&mut op)?;
    Ok(())
}

impl<MODE> File<MODE> {
    fn write_internal(&mut self, buffer: &[u8]) -> Result<(), Error> {
        let length = buffer.len();
        let mut op = Operation::Write(WriteArgs {
            fd: self.fd,
            buffer: &buffer[0],
            length,
        });

        let result = call_host(&mut op)?;

        if result != 0 {
            Err(Error::WriteError(result))
        } else {
            Ok(())
        }
    }

    fn read_internal(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let length = buffer.len();
        let mut op = Operation::Read(ReadArgs {
            fd: self.fd,
            buffer: &mut buffer[0],
            length,
        });

        let result = call_host(&mut op)?;

        if result == length {
            Err(Error::EndOfFile)
        } else {
            Ok(length - result)
        }
    }

    pub fn seek(&mut self, byte_offset: usize) -> Result<(), Error> {
        let mut op = Operation::Seek(SeekArgs {
            fd: self.fd,
            offset: byte_offset,
        });

        call_host(&mut op)?;
        Ok(())
    }

    pub fn length(&self) -> Result<usize, Error> {
        let mut op = Operation::Flen(FlenArgs { fd: self.fd });

        let result = call_host(&mut op)?;
        Ok(result as usize)
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
        let mut op = Operation::Close(CloseArgs { fd: self.fd });
        call_host(&mut op).ok();
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
