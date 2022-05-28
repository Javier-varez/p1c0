//! Ram File system that is loaded with the kernel. Used as the rootfs

use super::{
    cpio::{self, CpioHeader},
    Error, FileDescription, FileType, FilesystemDevice, FilesystemDriver, OpenMode, Result,
};
use crate::prelude::*;

/// This filesystem assumes that the order of records within the archive is depth first.
/// That ensures that we can find all the children of a directory node without iterating the
/// whole tree.
struct InitFsDevice {
    data: &'static [u8],
}

impl InitFsDevice {
    const fn new(data: &'static [u8]) -> Self {
        Self { data }
    }

    fn filetype_from_cpio_hdr(&self, header: &CpioHeader) -> Result<FileType> {
        match header.mode & super::permissions::S_IFMT {
            super::permissions::S_IFIFO => Ok(FileType::Fifo),
            super::permissions::S_IFDIR => Ok(FileType::Directory),
            super::permissions::S_IFREG => Ok(FileType::RegularFile),
            super::permissions::S_IFCHR => Ok(FileType::CharDevice),
            super::permissions::S_IFBLK => Ok(FileType::BlockDevice),
            super::permissions::S_IFLNK => Ok(FileType::SymbolicLink),
            super::permissions::S_IFSOCK => Ok(FileType::Socket),
            _ => {
                log_warning!("Invalid file mode found 0o{:o}", header.mode);
                Err(Error::InvalidFileDescription)
            }
        }
    }

    fn find_node(&self, path: &str) -> Option<FileDescription> {
        let path = path.strip_prefix('/').unwrap_or(path);

        let mut offset = 0;
        loop {
            match cpio::parse_entry(&self.data[offset..]) {
                Ok(Some(entry)) if entry.name == path => {
                    return Some(FileDescription {
                        block_offset: offset,
                        _inode_number: entry.inode as _,
                        filetype: self.filetype_from_cpio_hdr(&entry).ok()?,
                        mode: entry.mode,
                        group_id: entry.gid,
                        user_id: entry.uid,
                        size: entry.filesize as usize,
                        read_offset: 0,
                    });
                }
                Ok(Some(entry)) => {
                    offset += entry.next_entry_offset;
                }
                Ok(None) => {
                    return None;
                }
                Err(error) => {
                    panic!("Error parsing cpio entry: {:?}", error);
                }
            }
        }
    }
}

impl FilesystemDevice for InitFsDevice {
    fn open(&self, path: &str, mode: OpenMode) -> Result<FileDescription> {
        if mode != OpenMode::Read {
            return Err(Error::OperationNotSupported);
        }

        self.find_node(path).ok_or(Error::FileNotFound)
    }

    fn read(&self, fd: &mut FileDescription, buffer: &mut [u8]) -> Result<usize> {
        let header = cpio::parse_entry(&self.data[fd.block_offset..])
            .unwrap()
            .unwrap();
        let data_offset = fd.block_offset + header.data_offset;

        if fd.read_offset > fd.size {
            return Err(Error::EndOfFile);
        }

        let available_bytes = fd.size - fd.read_offset;

        let copy_size = if buffer.len() > available_bytes {
            available_bytes
        } else {
            buffer.len()
        };

        let offset = data_offset + fd.read_offset;
        buffer.copy_from_slice(&self.data[offset..offset + copy_size]);

        fd.read_offset += copy_size;
        Ok(copy_size)
    }

    fn close(&self, _fd: FileDescription) {
        // Nothing to do here
    }
}

struct InitFsDriver {}

impl FilesystemDriver for InitFsDriver {
    fn mount(
        &self,
        _target_path: &str,
        _source_path: Option<&str>,
        _options: &str,
    ) -> Result<Box<dyn FilesystemDevice>> {
        Err(Error::OperationNotSupported)
    }

    fn mount_from_static_data(&self, data: &'static [u8]) -> Result<Box<dyn FilesystemDevice>> {
        match cpio::parse_entry(data).map_err(|_| Error::InvalidFilesystem)? {
            Some(_) => Ok(Box::new(InitFsDevice::new(data))),
            None => {
                log_warning!("Empty initfs!");
                Err(Error::InvalidFilesystem)
            }
        }
    }
}

pub fn register_init_fs() {
    let driver = Box::new(InitFsDriver {});
    super::register_driver("initfs", driver);
}
