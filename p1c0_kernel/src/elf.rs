use crate::prelude::*;

macro_rules! read_elf64_half {
    ($buffer: expr, $offset: ident) => {
        $buffer[file_offsets::elf64::$offset] as Elf64_Half
            | ($buffer[file_offsets::elf64::$offset + 1] as Elf64_Half) << 8
    };
}

macro_rules! read_elf64_word {
    ($buffer: expr, $offset: ident) => {
        $buffer[file_offsets::elf64::$offset] as Elf64_Word
            | ($buffer[file_offsets::elf64::$offset + 1] as Elf64_Word) << 8
            | ($buffer[file_offsets::elf64::$offset + 2] as Elf64_Word) << 16
            | ($buffer[file_offsets::elf64::$offset + 3] as Elf64_Word) << 24
    };
}

macro_rules! read_elf64_off {
    ($buffer: expr, $offset: ident) => {
        $buffer[file_offsets::elf64::$offset] as Elf64_Off
            | ($buffer[file_offsets::elf64::$offset + 1] as Elf64_Off) << 8
            | ($buffer[file_offsets::elf64::$offset + 2] as Elf64_Off) << 16
            | ($buffer[file_offsets::elf64::$offset + 3] as Elf64_Off) << 24
            | ($buffer[file_offsets::elf64::$offset + 4] as Elf64_Off) << 32
            | ($buffer[file_offsets::elf64::$offset + 5] as Elf64_Off) << 40
            | ($buffer[file_offsets::elf64::$offset + 6] as Elf64_Off) << 48
            | ($buffer[file_offsets::elf64::$offset + 7] as Elf64_Off) << 56
    };
}

macro_rules! read_elf64_addr {
    ($buffer: expr, $offset: ident) => {
        $buffer[file_offsets::elf64::$offset] as Elf64_Addr
            | ($buffer[file_offsets::elf64::$offset + 1] as Elf64_Addr) << 8
            | ($buffer[file_offsets::elf64::$offset + 2] as Elf64_Addr) << 16
            | ($buffer[file_offsets::elf64::$offset + 3] as Elf64_Addr) << 24
            | ($buffer[file_offsets::elf64::$offset + 4] as Elf64_Addr) << 32
            | ($buffer[file_offsets::elf64::$offset + 5] as Elf64_Addr) << 40
            | ($buffer[file_offsets::elf64::$offset + 6] as Elf64_Addr) << 48
            | ($buffer[file_offsets::elf64::$offset + 7] as Elf64_Addr) << 56
    };
}

macro_rules! read_elf64_xword {
    ($buffer: expr, $offset: ident) => {
        $buffer[file_offsets::elf64::$offset] as Elf64_Xword
            | ($buffer[file_offsets::elf64::$offset + 1] as Elf64_Xword) << 8
            | ($buffer[file_offsets::elf64::$offset + 2] as Elf64_Xword) << 16
            | ($buffer[file_offsets::elf64::$offset + 3] as Elf64_Xword) << 24
            | ($buffer[file_offsets::elf64::$offset + 4] as Elf64_Xword) << 32
            | ($buffer[file_offsets::elf64::$offset + 5] as Elf64_Xword) << 40
            | ($buffer[file_offsets::elf64::$offset + 6] as Elf64_Xword) << 48
            | ($buffer[file_offsets::elf64::$offset + 7] as Elf64_Xword) << 56
    };
}

#[derive(Debug)]
pub enum Error {
    NotAnElfFile,
    InvalidElfClass(u8),
    InvalidElfEndianness(u8),
    InvalidElfType(Elf64_Half),
    InvalidElfMachine(Elf64_Half),
    InvalidPType(Elf64_Word),
    InvalidShType(Elf64_Word),
    InvalidSymbolType(u8),
    UnsupportedElfClass(EClass),
    UnsupportedElfEndianness(EData),
    NoMatchingSection,
}

#[derive(Clone)]
pub struct ElfParser<'a> {
    elf_data: &'a [u8],
    class: EClass,
    ty: EType,
    machine: EMachine,
}

impl<'a> ElfParser<'a> {
    pub fn from_slice(elf_data: &'a [u8]) -> Result<Self, Error> {
        const HEADER_LENGTH: usize = 16;
        if elf_data.len() < HEADER_LENGTH {
            log_error!("Not enough data");
            return Err(Error::NotAnElfFile);
        }

        const MAGIC0: u8 = 0x7f;
        const MAGIC1: u8 = b'E';
        const MAGIC2: u8 = b'L';
        const MAGIC3: u8 = b'F';

        // Read header
        if elf_data[file_offsets::E_MAGIC0] != MAGIC0
            || elf_data[file_offsets::E_MAGIC1] != MAGIC1
            || elf_data[file_offsets::E_MAGIC2] != MAGIC2
            || elf_data[file_offsets::E_MAGIC3] != MAGIC3
        {
            log_error!("Invalid ELF magic");
            return Err(Error::NotAnElfFile);
        }

        log_verbose!("Elf file found!");

        // Read the class to figure out the type of ELF we have
        let class: EClass = elf_data[file_offsets::E_CLASS].try_into()?;
        log_verbose!("Elf class {:?}", class);
        if !matches!(class, EClass::Elf64) {
            log_error!("Unsupported Elf class {:?}", class);
            return Err(Error::UnsupportedElfClass(class));
        }

        let data: EData = elf_data[file_offsets::E_DATA].try_into()?;
        log_verbose!("Elf data {:?}", data);
        if !matches!(data, EData::LittleEndian) {
            log_error!("Unsupported Elf endianness {:?}", data);
            return Err(Error::UnsupportedElfEndianness(data));
        }

        let ty: EType = read_elf64_half!(elf_data, E_TYPE).try_into()?;
        log_verbose!("Elf type {:?}", ty);

        let machine: EMachine = read_elf64_half!(elf_data, E_MACHINE).try_into()?;
        log_verbose!("Elf machine {:?}", machine);

        Ok(Self {
            elf_data,
            class,
            ty,
            machine,
        })
    }

    pub fn elf_type(&self) -> EType {
        self.ty
    }

    pub fn machine(&self) -> EMachine {
        self.machine
    }

    #[must_use]
    pub fn entry_point(&self) -> Elf64_Addr {
        match self.class {
            EClass::Elf32 => unimplemented!(),
            EClass::Elf64 => {
                let entry: Elf64_Addr = read_elf64_addr!(self.elf_data, E_ENTRY);
                log_verbose!("Entrypoint 0x{:x}", entry);
                entry
            }
        }
    }

    #[must_use]
    pub fn program_header_iter(&self) -> ProgramHeaderIter<'a> {
        match self.class {
            EClass::Elf32 => unimplemented!(),
            EClass::Elf64 => {
                let phoff: Elf64_Off = read_elf64_off!(self.elf_data, E_PHOFF);
                let phsize: Elf64_Half = read_elf64_half!(self.elf_data, E_PHENTSIZE);
                let phnum: Elf64_Half = read_elf64_half!(self.elf_data, E_PHNUM);
                log_verbose!(
                    "Program header offset 0x{:x}, size 0x{:x}, num_entries {}",
                    phoff,
                    phsize,
                    phnum
                );

                let start = phoff as usize;
                let end = start + (phsize as usize * phnum as usize);

                ProgramHeaderIter {
                    pheader_data: &self.elf_data[start..end],
                    num_entries: phnum,
                    entry_size: phsize,
                    current_entry: 0,
                }
            }
        }
    }

    #[must_use]
    pub fn section_header_iter(&self) -> SectionHeaderIter<'a> {
        match self.class {
            // No need to support ELF32 at this point
            EClass::Elf32 => unimplemented!(),
            EClass::Elf64 => {
                let shoff: Elf64_Off = read_elf64_off!(self.elf_data, E_SHOFF);
                let shsize: Elf64_Half = read_elf64_half!(self.elf_data, E_SHENTSIZE);
                let shnum: Elf64_Half = read_elf64_half!(self.elf_data, E_SHNUM);
                log_verbose!(
                    "Section header offset 0x{:x}, size 0x{:x}, num_entries {}",
                    shoff,
                    shsize,
                    shnum
                );

                let start = shoff as usize;
                let end = start + (shsize as usize * shnum as usize);

                SectionHeaderIter {
                    section_header_data: &self.elf_data[start..end],
                    num_entries: shnum,
                    entry_size: shsize,
                    current_entry: 0,
                }
            }
        }
    }

    pub fn get_segment_data(&self, program_header: &ProgramHeader) -> &[u8] {
        let file_offset = program_header.file_offset() as usize;
        let file_size = program_header.filesize() as usize;
        &self.elf_data[file_offset..file_offset + file_size]
    }

    fn get_str_table_name_section(&self) -> Option<SectionHeader> {
        let index = read_elf64_half!(self.elf_data, E_SHSTRNDX) as usize;
        if index != SHN_UNDEF {
            log_verbose!("str_table index {}", index);
            self.section_header_iter().nth(index)
        } else {
            None
        }
    }

    fn find_section_name_by_index(&self, name_index: usize) -> Option<&str> {
        let section = self.get_str_table_name_section()?;
        // Double check the section type
        if !matches!(section.ty().ok()?, ShType::StrTab) {
            log_warning!("Section does not have StrTab type");
            return None;
        }

        let offset = section.offset() as usize + name_index;
        // Now get the string from the index
        let data = &self.elf_data[offset..];
        let mut length = 0;
        while data[length] != b'\0' {
            length += 1;
        }
        let data = &self.elf_data[offset..offset + length];
        let string = core::str::from_utf8(data).ok()?;

        Some(string)
    }

    pub fn matching_section_name(
        &self,
        program_header: &ProgramHeader,
    ) -> Result<Option<&str>, Error> {
        log_verbose!(
            "Finding matching name for pheader with vaddr 0x{:x}",
            program_header.vaddr()
        );

        for section in self.section_header_iter() {
            if matches!(section.ty()?, ShType::Progbits)
                && section.vaddr() == program_header.vaddr()
            {
                log_verbose!("Found matching section by vaddr");
                // Matching section found
                let name_idx = section.name_idx() as usize;
                return Ok(self.find_section_name_by_index(name_idx));
            }
        }

        Err(Error::NoMatchingSection)
    }

    pub fn symbol_table_iter(&self) -> Option<SymbolTableIter> {
        if let Some(symtab) = self
            .section_header_iter()
            .find(|section| matches!(section.ty(), Ok(ShType::SymTab)))
        {
            let symbol_table_offset = symtab.offset() as usize;
            let symbol_table_size = symtab.size() as usize;

            if let Some(strtab) = self.section_header_iter().nth(symtab.link() as usize) {
                let symbol_strtable_offset = strtab.offset() as usize;
                let symbol_strtable_size = strtab.size() as usize;

                // This is data with symbol entries
                let symbol_table_data =
                    &self.elf_data[symbol_table_offset..symbol_table_offset + symbol_table_size];

                let symbol_strtable_data = &self.elf_data
                    [symbol_strtable_offset..symbol_strtable_offset + symbol_strtable_size];

                let iter = SymbolTableIter {
                    data: symbol_table_data,
                    strdata: symbol_strtable_data,
                    num_entries: symtab.size() as usize / symtab.entry_size() as usize,
                    entry_size: symtab.entry_size() as usize,
                    index: 0,
                };

                return Some(iter);
            }
        }

        None
    }
}

pub struct ProgramHeader<'a> {
    pheader_data: &'a [u8],
}

impl<'a> ProgramHeader<'a> {
    pub fn ty(&self) -> Result<PtType, Error> {
        let p_type: PtType = read_elf64_word!(self.pheader_data, P_TYPE).try_into()?;
        Ok(p_type)
    }

    pub fn file_offset(&self) -> Elf64_Off {
        read_elf64_off!(self.pheader_data, P_OFFSET)
    }

    pub fn vaddr(&self) -> Elf64_Addr {
        read_elf64_addr!(self.pheader_data, P_VADDR)
    }

    pub fn paddr(&self) -> Elf64_Addr {
        read_elf64_addr!(self.pheader_data, P_PADDR)
    }

    pub fn memsize(&self) -> Elf64_Xword {
        read_elf64_xword!(self.pheader_data, P_MEMSIZE)
    }

    pub fn filesize(&self) -> Elf64_Xword {
        read_elf64_xword!(self.pheader_data, P_FILESIZE)
    }

    pub fn permissions(&self) -> Permissions {
        pub const PF_R: Elf64_Word = 4;
        pub const PF_W: Elf64_Word = 2;
        pub const PF_X: Elf64_Word = 1;

        let flags = read_elf64_word!(self.pheader_data, P_FLAGS);
        let read = (flags & PF_R) != 0;
        let write = (flags & PF_W) != 0;
        let exec = (flags & PF_X) != 0;
        Permissions { read, write, exec }
    }
}

pub struct SectionHeader<'a> {
    section_header_data: &'a [u8],
}

impl<'a> SectionHeader<'a> {
    pub fn name_idx(&self) -> Elf64_Word {
        read_elf64_word!(self.section_header_data, SH_NAME)
    }

    pub fn ty(&self) -> Result<ShType, Error> {
        let sh_type: ShType = read_elf64_word!(self.section_header_data, SH_TYPE).try_into()?;
        Ok(sh_type)
    }

    pub fn vaddr(&self) -> Elf64_Addr {
        read_elf64_addr!(self.section_header_data, SH_ADDR)
    }

    pub fn offset(&self) -> Elf64_Off {
        read_elf64_off!(self.section_header_data, SH_OFFSET)
    }

    pub fn size(&self) -> Elf64_Xword {
        read_elf64_xword!(self.section_header_data, SH_SIZE)
    }

    pub fn link(&self) -> Elf64_Word {
        read_elf64_word!(self.section_header_data, SH_LINK)
    }

    pub fn entry_size(&self) -> Elf64_Xword {
        read_elf64_xword!(self.section_header_data, SH_ENTSIZE)
    }
}

pub struct ProgramHeaderIter<'a> {
    pheader_data: &'a [u8],
    num_entries: Elf64_Half,
    entry_size: Elf64_Half,
    current_entry: Elf64_Half,
}

impl<'a> Iterator for ProgramHeaderIter<'a> {
    type Item = ProgramHeader<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_entry < self.num_entries {
            let start = self.current_entry as usize * self.entry_size as usize;
            let end = start + self.entry_size as usize;
            let data = &self.pheader_data[start..end];
            self.current_entry += 1;
            return Some(ProgramHeader { pheader_data: data });
        }
        None
    }
}

pub struct SectionHeaderIter<'a> {
    section_header_data: &'a [u8],
    num_entries: Elf64_Half,
    entry_size: Elf64_Half,
    current_entry: Elf64_Half,
}

impl<'a> Iterator for SectionHeaderIter<'a> {
    type Item = SectionHeader<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.current_entry < self.num_entries {
            let start = self.current_entry as usize * self.entry_size as usize;
            let end = start + self.entry_size as usize;
            let data = &self.section_header_data[start..end];
            self.current_entry += 1;
            return Some(SectionHeader {
                section_header_data: data,
            });
        }
        None
    }
}

pub struct SymbolEntry<'a> {
    data: &'a [u8],
    strdata: &'a [u8],
}

impl<'a> SymbolEntry<'a> {
    pub fn ty(&self) -> Result<SymbolType, Error> {
        self.data[file_offsets::elf64::ST_INFO].try_into()
    }

    pub fn value(&self) -> Elf64_Addr {
        read_elf64_addr!(self.data, ST_VALUE)
    }

    pub fn size(&self) -> Elf64_Xword {
        read_elf64_xword!(self.data, ST_SIZE)
    }

    pub fn name(&self) -> Option<&str> {
        let name_idx = read_elf64_word!(self.data, ST_NAME) as usize;

        // Now get the string from the index
        let strdata = &self.strdata[name_idx..];
        let mut length = 0;
        while strdata[length] != b'\0' {
            length += 1;
        }

        let strdata = &strdata[..length];
        core::str::from_utf8(strdata).ok()
    }
}

pub struct SymbolTableIter<'a> {
    data: &'a [u8],
    strdata: &'a [u8],
    entry_size: usize,
    num_entries: usize,
    index: usize,
}

impl<'a> Iterator for SymbolTableIter<'a> {
    type Item = SymbolEntry<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.num_entries {
            return None;
        }

        let symbol_entry_data =
            &self.data[self.index * self.entry_size..(self.index + 1) * self.entry_size];
        self.index += 1;
        Some(SymbolEntry {
            data: symbol_entry_data,
            strdata: self.strdata,
        })
    }
}

const SHN_UNDEF: usize = 0;

mod file_offsets {
    pub const E_MAGIC0: usize = 0x00;
    pub const E_MAGIC1: usize = 0x01;
    pub const E_MAGIC2: usize = 0x02;
    pub const E_MAGIC3: usize = 0x03;
    pub const E_CLASS: usize = 0x04;
    pub const E_DATA: usize = 0x05;

    pub mod elf64 {
        pub const E_TYPE: usize = 16;
        pub const E_MACHINE: usize = 18;
        pub const E_ENTRY: usize = 0x18;
        pub const E_PHOFF: usize = 0x20;
        pub const E_SHOFF: usize = 0x28;
        pub const E_PHENTSIZE: usize = 0x36;
        pub const E_PHNUM: usize = 0x38;
        pub const E_SHENTSIZE: usize = 0x3A;
        pub const E_SHNUM: usize = 0x3C;
        pub const E_SHSTRNDX: usize = 0x3E;

        // Program header
        pub const P_TYPE: usize = 0x00;
        pub const P_FLAGS: usize = 0x04;
        pub const P_OFFSET: usize = 0x08;
        pub const P_VADDR: usize = 0x10;
        pub const P_PADDR: usize = 0x18;
        pub const P_MEMSIZE: usize = 0x20;
        pub const P_FILESIZE: usize = 0x28;

        // Section header
        pub const SH_NAME: usize = 0x00;
        pub const SH_TYPE: usize = 0x04;
        pub const SH_ADDR: usize = 0x10;
        pub const SH_OFFSET: usize = 0x18;
        pub const SH_SIZE: usize = 0x20;
        pub const SH_LINK: usize = 0x28;
        pub const SH_ENTSIZE: usize = 0x38;

        // Symbol table entry
        pub const ST_NAME: usize = 0x00;
        pub const ST_INFO: usize = 0x04;
        pub const ST_VALUE: usize = 0x08;
        pub const ST_SIZE: usize = 0x10;
    }
}

#[allow(non_camel_case_types)]
type Elf64_Addr = u64;
#[allow(non_camel_case_types)]
type Elf64_Off = u64;
#[allow(non_camel_case_types)]
type Elf64_Half = u16;
#[allow(non_camel_case_types)]
type Elf64_Word = u32;
#[allow(non_camel_case_types)]
type Elf64_Xword = u64;

macro_rules! define_enum {
    {
        $name: ident,
        $inner_type: ty,
        [
            $($field_name: ident = $field_value: literal),+
        ],
        $error_ident: ident
    } => {
        #[derive(Debug, Copy, Clone)]
        pub enum $name {
            $($field_name),+
        }

        impl TryFrom<$inner_type> for $name {
            type Error = Error;
            fn try_from(value: $inner_type) -> Result<Self, Self::Error> {
                match value {
                    $( $field_value => Ok($name::$field_name),)+
                    _ => Err(Error::$error_ident(value)),
                }
            }
        }
    };
}

define_enum! {
    EClass, u8,
    [
        Elf32 = 1,
        Elf64 = 2
    ],
    InvalidElfClass
}

define_enum! {
    EData, u8,
    [
        LittleEndian = 1,
        BigEndian = 2
    ],
    InvalidElfEndianness
}

define_enum! {
    EType, Elf64_Half,
    [
        Relocatable = 1,
        Executable = 2,
        SharedObject = 3,
        Core = 4
    ],
    InvalidElfType
}

define_enum! {
    EMachine, Elf64_Half,
    [
        AARCH64 = 183
    ],
    InvalidElfMachine
}

define_enum! {
    PtType, Elf64_Word,
    [
        Null = 0,
        Load = 1,
        Dynamic = 2,
        Interpreter = 3,
        Note = 4,
        Shlib = 5,
        Phdr = 6,
        Tls = 7
    ],
    InvalidPType
}

define_enum! {
    ShType, Elf64_Word,
    [
        Null = 0,
        Progbits = 1,
        SymTab = 2,
        StrTab = 3,
        RelA = 4,
        Hash = 5,
        Dynamic = 6,
        Note = 7,
        NoBits = 8,
        Rel = 9
    ],
    InvalidShType
}

#[derive(Debug, Copy, Clone)]
pub enum SymbolType {
    NoType = 0,
    Object = 1,
    Function = 2,
    Section = 3,
    File = 4,
    Common = 5,
    Tls = 6,
    LoOS = 10,
    HiOS = 12,
    LoProc = 13,
    HiProc = 15,
}

impl TryFrom<u8> for SymbolType {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let symbol_type = value & 0xf;
        match symbol_type {
            0 => Ok(SymbolType::NoType),
            1 => Ok(SymbolType::Object),
            2 => Ok(SymbolType::Function),
            3 => Ok(SymbolType::Section),
            4 => Ok(SymbolType::File),
            5 => Ok(SymbolType::Common),
            6 => Ok(SymbolType::Tls),
            10 => Ok(SymbolType::LoOS),
            12 => Ok(SymbolType::HiOS),
            13 => Ok(SymbolType::LoProc),
            15 => Ok(SymbolType::HiProc),
            _ => Err(Error::InvalidSymbolType(symbol_type)),
        }
    }
}

pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}
