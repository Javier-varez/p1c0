mod cpio;
mod initfs;

use crate::prelude::*;
use crate::sync::spinlock::RwSpinLock;

use p1c0_macros::initcall;

type Result<T> = ::core::result::Result<T, Error>;

pub mod permissions {
    // User
    pub const S_IREAD: u32 = 0o400;
    pub const S_IWRITE: u32 = 0o200;
    pub const S_IEXEC: u32 = 0o100;

    pub const S_IRUSR: u32 = 0o400;
    pub const S_IWUSR: u32 = 0o200;
    pub const S_IXUSR: u32 = 0o100;
    pub const S_IRWXU: u32 = 0o700;

    // Group
    pub const S_IRGRP: u32 = 0o040;
    pub const S_IWGRP: u32 = 0o020;
    pub const S_IXGRP: u32 = 0o010;
    pub const S_IRWXG: u32 = 0o070;

    // Others
    pub const S_IROTH: u32 = 0o004;
    pub const S_IWOTH: u32 = 0o002;
    pub const S_IXOTH: u32 = 0o001;
    pub const S_IRWXO: u32 = 0o007;

    // Set user id
    pub const S_ISUID: u32 = 0o4000;
    // Set group id
    pub const S_ISGID: u32 = 0o2000;
    // Sticky bit
    pub const S_ISVTX: u32 = 0o1000;

    // File type mask
    pub const S_IFMT: u32 = 0o0170000;
    // FIFO type
    pub const S_IFIFO: u32 = 0o0010000;
    // Char file type
    pub const S_IFCHR: u32 = 0o0020000;
    // Dir type
    pub const S_IFDIR: u32 = 0o0040000;
    // Block file type
    pub const S_IFBLK: u32 = 0o0060000;
    // Regular file type
    pub const S_IFREG: u32 = 0o0100000;
    // Symbolic link file type
    pub const S_IFLNK: u32 = 0o0120000;
    // Socket file type
    pub const S_IFSOCK: u32 = 0o0140000;
}

/// Type-erased Filesystem specific error trait
pub trait FsError: core::fmt::Debug + core::fmt::Display {
    fn source(&self) -> Option<&(dyn FsError + 'static)>;
    fn description(&self) -> &str;
    fn cause(&self) -> Option<&dyn FsError>;
}

#[derive(Debug)]
pub enum Error {
    /// The FS could not be mounted because there is no matching FS driver for the requested fs_type
    NoMatchingDriverFound,
    /// The operation is not supported by the driver
    OperationNotSupported,
    /// The filesystem cannot be mounted because it is invalid (invalid magic or similar)
    InvalidFilesystem,
    /// The provided file description does not match the expected type or is not valid
    InvalidFileDescription,
    /// The Requested file could not be found
    FileNotFound,
    /// No more data to read
    EndOfFile,
    /// Type-erased Filesystem specific error
    FsSpecific(Box<dyn FsError>),
}

pub trait FilesystemDriver {
    /// This API can be used by filesystems to mount a FS from a source path.
    /// Some FS don't require a source path (such is the case of devfs or procfs).
    ///
    /// The default implementation returns operation not supported.
    fn mount(
        &self,
        _target_path: &str,
        _source_path: Option<&str>,
        _options: &str,
    ) -> Result<Box<dyn FilesystemDevice>> {
        Err(Error::OperationNotSupported)
    }

    /// This API can be used by filesystems to mount a FS from statically available data.
    ///
    /// The default implementation returns operation not supported
    fn mount_from_static_data(&self, _data: &'static [u8]) -> Result<Box<dyn FilesystemDevice>> {
        Err(Error::OperationNotSupported)
    }
}

// Registered filesystem drivers. To be filled during init via register_driver
static FS_DRIVERS: RwSpinLock<FlatMap<String, Box<dyn FilesystemDriver>>> =
    RwSpinLock::new(FlatMap::new_no_capacity());

// Registered filesystem drivers. To be filled during init via register_driver
static VFS: RwSpinLock<VirtualFileSystem> = RwSpinLock::new(VirtualFileSystem::new());

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum OpenMode {
    Read,
    Write,
    Append,
    ReadWrite,
    ReadAppend,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FileType {
    Directory,
    RegularFile,
    SymbolicLink,
    CharDevice,
    BlockDevice,
    Fifo,
    Socket,
}

#[derive(Debug)]
pub struct FileDescription {
    pub filetype: FileType,
    pub mode: u32,
    pub user_id: u32,
    pub group_id: u32,
    pub size: usize,
    _inode_number: u64,
    block_offset: usize,
    read_offset: usize,
}

pub enum SeekMode {
    Start(usize),
    CurrentPosition(usize),
    End,
}

pub trait FilesystemDevice {
    fn open(&self, path: &str, mode: OpenMode) -> Result<FileDescription>;
    fn read(&self, fd: &mut FileDescription, buffer: &mut [u8]) -> Result<usize>;
    fn close(&self, fd: FileDescription);
}

pub struct VirtualFileSystem {
    rootfs: Option<Box<dyn FilesystemDevice>>,
}

impl VirtualFileSystem {
    const fn new() -> Self {
        Self { rootfs: None }
    }

    fn mount_rootfs(&mut self, data: &'static [u8]) -> Result<()> {
        if let Some(fs_driver) = FS_DRIVERS.lock_read().lookup("initfs") {
            let device = fs_driver.mount_from_static_data(data)?;
            self.rootfs.replace(device);
            Ok(())
        } else {
            Err(Error::NoMatchingDriverFound)
        }
    }

    pub fn open(path: &str, mode: OpenMode) -> Result<FileDescription> {
        VFS.lock_read().rootfs.as_ref().unwrap().open(path, mode)
    }

    pub fn read(fd: &mut FileDescription, buffer: &mut [u8]) -> Result<usize> {
        VFS.lock_read().rootfs.as_ref().unwrap().read(fd, buffer)
    }

    pub fn fseek(file: &mut FileDescription, seek_mode: SeekMode) -> Result<()> {
        let requested_offset = match seek_mode {
            SeekMode::Start(offset) => offset,
            SeekMode::CurrentPosition(offset) => file.read_offset + offset,
            SeekMode::End => file.size,
        };

        if requested_offset > file.size {
            return Err(Error::EndOfFile);
        }

        file.read_offset = requested_offset;
        Ok(())
    }

    pub fn close(fd: FileDescription) {
        VFS.lock_read().rootfs.as_ref().unwrap().close(fd);
    }
}

pub struct Path<'a> {
    path: &'a str,
}

impl<'a> TryFrom<&'a str> for Path<'a> {
    type Error = ();
    fn try_from(string: &'a str) -> ::core::result::Result<Self, ()> {
        let string = string.trim();

        // This must be an absolute string, since we don't have enough context to know the cwd here.
        if !string.starts_with('/') {
            return Err(());
        }

        let path = string.trim_start_matches('/').trim_end_matches('/');
        Ok(Self { path })
    }
}

impl<'a> Path<'a> {
    #[must_use]
    pub fn iter(&self) -> PathIter {
        PathIter { path: self.path }
    }
}

pub struct PathIter<'a> {
    path: &'a str,
}

impl<'a> Iterator for PathIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.path.is_empty() {
            return None;
        }

        loop {
            if let Some((component, path)) = self.path.split_once('/') {
                self.path = path;
                if component.is_empty() {
                    // Ignore repeated /
                    continue;
                }
                return Some(component);
            } else {
                if !self.path.is_empty() {
                    let component = self.path;
                    self.path = "";
                    return Some(component);
                }
                return None;
            }
        }
    }
}

pub fn register_driver(name: &str, driver: Box<dyn FilesystemDriver>) {
    log_debug!("Registering FS driver with name {}", name);
    if let Err(flat_map::Error::KeyAlreadyPresentInMap) =
        FS_DRIVERS.lock_write().insert_with_strategy(
            name.to_string(),
            driver,
            flat_map::InsertStrategy::NoReplaceResize,
        )
    {
        panic!(
            "Tried to register two fs drivers with the same key `{}`",
            name
        );
    }
}

/// This is the static CPIO archive for the Root FS. It is built with the build process and packaged
/// into a CPIO file
static CPIO_ARCHIVE: &[u8] = include_bytes!("../../build/rootfs.cpio");

#[initcall(priority = 1)]
pub fn register_filesystems() {
    initfs::register_init_fs();
}

#[initcall]
pub fn mount_rootfs() {
    VFS.lock_write().mount_rootfs(CPIO_ARCHIVE).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn can_construct_path() {
        let path = Path::try_from("/some/path").unwrap();

        let components: Vec<String> = path.iter().map(|c| c.to_string()).collect();
        assert_eq!(components, vec!["some", "path"]);
    }

    #[test]
    fn cannot_construct_relative_path_from_str() {
        assert!(Path::try_from("some/path").is_err());
    }

    #[test]
    fn path_ignores_duplicated_slash() {
        let path = Path::try_from("//some//path").unwrap();

        let components: Vec<String> = path.iter().map(|c| c.to_string()).collect();
        assert_eq!(components, vec!["some", "path"]);
    }

    #[test]
    fn path_removes_trailing_slash() {
        let path = Path::try_from("/some/path/").unwrap();

        let components: Vec<String> = path.iter().map(|c| c.to_string()).collect();
        assert_eq!(components, vec!["some", "path"]);
    }

    #[test]
    fn path_can_contain_symbols() {
        let path = Path::try_from("/some/path/file.txt").unwrap();

        let components: Vec<String> = path.iter().map(|c| c.to_string()).collect();
        assert_eq!(components, vec!["some", "path", "file.txt"]);
    }
}
