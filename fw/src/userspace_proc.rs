use p1c0_kernel::{
    filesystem::{self, OpenMode, VirtualFileSystem},
    prelude::*,
    process,
};

#[derive(Debug)]
pub enum Error {
    ProcessError(process::Error),
    FsError(filesystem::Error),
}

impl From<process::Error> for Error {
    fn from(err: process::Error) -> Self {
        Error::ProcessError(err)
    }
}

impl From<filesystem::Error> for Error {
    fn from(err: filesystem::Error) -> Self {
        Error::FsError(err)
    }
}

pub fn create_process(filename: &str, aslr: usize) -> Result<(), Error> {
    let mut file = VirtualFileSystem::open(filename, OpenMode::Read)?;
    let mut elf_data = vec![];
    elf_data.resize(file.size, 0);

    VirtualFileSystem::read(&mut file, &mut elf_data[..])?;
    VirtualFileSystem::close(file);

    process::new_from_elf_data(elf_data, aslr)?;
    Ok(())
}
