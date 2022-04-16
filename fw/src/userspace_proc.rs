use p1c0_kernel::{
    elf,
    filesystem::{self, OpenMode, VirtualFileSystem},
    memory::{address::VirtualAddress, Permissions},
    prelude::*,
    process,
};

#[derive(Debug)]
pub enum Error {
    ProcessError(process::Error),
    ElfError(elf::Error),
    FsError(filesystem::Error),
    FileNotExecutable,
    UnalignedVA(*const u8),
}

impl From<process::Error> for Error {
    fn from(err: process::Error) -> Self {
        Error::ProcessError(err)
    }
}

impl From<elf::Error> for Error {
    fn from(err: elf::Error) -> Self {
        Error::ElfError(err)
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

    let elf = elf::ElfParser::from_slice(&elf_data[..])?;

    if !matches!(
        elf.elf_type(),
        elf::EType::Executable | elf::EType::SharedObject
    ) {
        log_warning!("Elf file is not executable, bailing");
        return Err(Error::FileNotExecutable);
    }

    let mut process_builder = process::Builder::new();
    for header in elf.program_header_iter() {
        let header_type = header.ty()?;
        if matches!(header_type, elf::PtType::Load) {
            log_debug!(
                "Vaddr 0x{:x}, Paddr 0x{:x}, Memsize {} Filesize {}",
                header.vaddr(),
                header.paddr(),
                header.memsize(),
                header.filesize()
            );

            let vaddr = (header.vaddr() as usize + aslr) as *const _;
            let vaddr =
                VirtualAddress::try_from_ptr(vaddr).map_err(|_| Error::UnalignedVA(vaddr))?;

            let segment_data = elf.get_segment_data(&header);

            let permissions = match header.permissions() {
                elf::Permissions {
                    read: true,
                    write: true,
                    exec: false,
                } => Permissions::RW,
                elf::Permissions {
                    read: true,
                    write: false,
                    exec: false,
                } => Permissions::RO,
                elf::Permissions {
                    read: _,
                    write: false,
                    exec: true,
                } => Permissions::RX,
                elf::Permissions {
                    read: true,
                    write: true,
                    exec: true,
                } => Permissions::RWX,
                elf::Permissions { read, write, exec } => {
                    let read = if read { "R" } else { "-" };
                    let write = if write { "W" } else { "-" };
                    let exec = if exec { "X" } else { "-" };
                    panic!(
                        "Unsupported set of permissions found in elf {}{}{}",
                        read, write, exec
                    );
                }
            };

            process_builder.map_section(
                elf.matching_section_name(&header)?.unwrap_or(""),
                vaddr,
                header.memsize() as usize,
                segment_data,
                permissions,
            )?;
        } else {
            log_warning!("Unhandled ELF program header with type {:?}", header_type);
        }
    }

    let vaddr = (elf.entry_point() as usize + aslr) as *const _;
    let entry_point = VirtualAddress::new_unaligned(vaddr);
    let base_address = VirtualAddress::new_unaligned(aslr as *const _);
    process_builder.start(entry_point, base_address, elf_data)?;
    Ok(())
}
