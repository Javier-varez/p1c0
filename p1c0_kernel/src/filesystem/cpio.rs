use crate::prelude::*;

#[derive(Debug)]
pub enum Error {
    HeaderTooSmall,
    InvalidMagic,
    CouldNotParse,
}

type Result<T> = core::result::Result<T, Error>;

const HEADER_SIZE_BYTES: usize = 110;
const MAGIC_STR: &str = "070701";

#[derive(Debug)]
pub struct CpioHeader<'a> {
    pub inode: u32,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub nlink: u32,
    pub mtime: u32,
    pub filesize: u32,
    pub dev_major: u32,
    pub dev_minor: u32,
    pub rdev_major: u32,
    pub rdev_minor: u32,
    pub namesize: u32,
    pub name: &'a str,
    pub data_offset: usize,
    pub next_entry_offset: usize,
}

mod header_offsets {
    pub const INODE: usize = 6;
    pub const MODE: usize = 14;
    pub const UID: usize = 22;
    pub const GID: usize = 30;
    pub const NLINK: usize = 38;
    pub const MTIME: usize = 46;
    pub const FILESIZE: usize = 54;
    pub const DEV_MAJOR: usize = 62;
    pub const DEV_MINOR: usize = 70;
    pub const RDEV_MAJOR: usize = 78;
    pub const RDEV_MINOR: usize = 86;
    pub const NAMESIZE: usize = 94;
    pub const CHECK: usize = 102;
}

fn parse_hex32(data: &[u8]) -> Result<u32> {
    if data.len() != 8 {
        return Err(Error::CouldNotParse);
    }

    let mut value = 0u32;
    for nibble_char in data {
        let nibble = if (*nibble_char >= b'0') && (*nibble_char <= b'9') {
            *nibble_char - b'0'
        } else if (*nibble_char >= b'a') && (*nibble_char <= b'f') {
            *nibble_char - b'a' + 10
        } else if (*nibble_char >= b'A') && (*nibble_char <= b'F') {
            *nibble_char - b'A' + 10
        } else {
            return Err(Error::CouldNotParse);
        };
        value = (value << 4) + nibble as u32;
    }
    Ok(value)
}

macro_rules! parse_header_field {
    ($data: ident, $field: ident) => {
        parse_hex32(&$data[header_offsets::$field..(header_offsets::$field + 8)])
    };
}

/// Returns the current entry and the offset to the next entry
pub fn parse_entry(data: &[u8]) -> Result<Option<CpioHeader<'_>>> {
    if data.len() < HEADER_SIZE_BYTES {
        log_warning!("Cpio header is smaller than expected!");
        return Err(Error::HeaderTooSmall);
    }

    let magic_data = &data[..MAGIC_STR.len()];
    let magic_str = core::str::from_utf8(magic_data).map_err(|_| {
        log_warning!("Cannot parse data as magic");
        Error::InvalidMagic
    })?;

    // Check the magic bits here
    if magic_str != MAGIC_STR {
        log_warning!("Invalid cpio magic");
        return Err(Error::InvalidMagic);
    }

    // Make sure check is 0
    let check = parse_header_field!(data, CHECK)?;
    if check != 0 {
        return Err(Error::CouldNotParse);
    }

    let namesize = parse_header_field!(data, NAMESIZE)?;
    let filesize = parse_header_field!(data, FILESIZE)?;

    // Align header size to 4 bytes
    let name_offset = HEADER_SIZE_BYTES;
    let data_offset = (name_offset + namesize as usize + 3) & !3;
    let next_entry_offset = (data_offset + filesize as usize + 3) & !3;

    let name = core::str::from_utf8(&data[name_offset..(name_offset + namesize as usize - 1)])
        .map_err(|_| Error::CouldNotParse)?;

    // Strip ./ and / from name
    let name = name.strip_prefix("./").unwrap_or(name);
    let name = name.strip_prefix('/').unwrap_or(name);

    let header = CpioHeader {
        inode: parse_header_field!(data, INODE)?,
        mode: parse_header_field!(data, MODE)?,
        uid: parse_header_field!(data, UID)?,
        gid: parse_header_field!(data, GID)?,
        nlink: parse_header_field!(data, NLINK)?,
        mtime: parse_header_field!(data, MTIME)?,
        filesize,
        dev_major: parse_header_field!(data, DEV_MAJOR)?,
        dev_minor: parse_header_field!(data, DEV_MINOR)?,
        rdev_major: parse_header_field!(data, RDEV_MAJOR)?,
        rdev_minor: parse_header_field!(data, RDEV_MINOR)?,
        namesize,
        name,
        data_offset,
        next_entry_offset,
    };

    if name == "TRAILER!!!" {
        // This is the last entry
        return Ok(None);
    }

    Ok(Some(header))
}
