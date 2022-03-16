use p1c0_kernel::{
    elf, log_debug, log_warning,
    memory::{address::VirtualAddress, Permissions},
    process,
};

#[derive(Debug)]
pub enum Error {
    ProcessError(process::Error),
    ElfError(elf::Error),
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

const ELF: &[u8] = include_bytes!("../../userspace_test/build/userspace_test");

pub fn create_process(aslr: usize) -> Result<(), Error> {
    let elf = elf::ElfParser::from_slice(ELF)?;

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

            process_builder.map_section(
                elf.matching_section_name(&header)?.unwrap_or(""),
                vaddr,
                header.memsize() as usize,
                segment_data,
                Permissions::RWX, // TODO(javier-varez): Use proper permissions
            )?;
        } else {
            log_warning!("Unhandled ELF program header with type {:?}", header_type);
        }
    }

    let vaddr = (elf.entry_point() as usize + aslr) as *const _;
    let entry = VirtualAddress::new_unaligned(vaddr);
    process_builder.start(entry, VirtualAddress::new_unaligned(aslr as *const _))?;

    Ok(())
}
