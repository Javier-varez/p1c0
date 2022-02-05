extern crate alloc;

use super::{
    address::{Address, LogicalAddress, VirtualAddress},
    physical_page_allocator::PhysicalPage,
    Attributes, Permissions,
};
use crate::println;

use heapless::String;

use alloc::vec;
use alloc::vec::Vec;
use core::str::FromStr;

const MAX_NAME_LENGTH: usize = 32;

#[derive(Clone, Debug)]
pub enum Error {
    MemoryRangeNotFound(String<MAX_NAME_LENGTH>),
    MemoryRangeAlreadyExists(String<MAX_NAME_LENGTH>),
    MemoryRangeOverlaps(String<MAX_NAME_LENGTH>),
    NameTooLong,
}

pub(super) struct VirtualMemoryRange {
    pub va: VirtualAddress,
    pub size_bytes: usize,
    pub name: String<MAX_NAME_LENGTH>,
    pub _attributes: Attributes,
    pub _permissions: Permissions,
    pub _pages: Vec<PhysicalPage>,
    // We can later add operations based on backed descriptors here
}

pub(super) struct LogicalMemoryRange {
    pub la: LogicalAddress,
    pub size_bytes: usize,
    pub name: heapless::String<32>,
    pub attributes: Attributes,
    pub permissions: Permissions,
    // Pages are implied in this case
}

pub(super) enum GenericMemoryRange {
    Logical(LogicalMemoryRange),
    Virtual(VirtualMemoryRange),
}

impl From<LogicalMemoryRange> for GenericMemoryRange {
    fn from(logical_range: LogicalMemoryRange) -> Self {
        Self::Logical(logical_range)
    }
}

impl From<VirtualMemoryRange> for GenericMemoryRange {
    fn from(virtual_range: VirtualMemoryRange) -> Self {
        Self::Virtual(virtual_range)
    }
}

pub trait MemoryRange {
    fn virtual_address(&self) -> VirtualAddress;
    fn size_bytes(&self) -> usize;

    fn end_virtual_address(&self) -> VirtualAddress {
        unsafe { self.virtual_address().offset(self.size_bytes()) }
    }

    fn overlaps(&self, va: VirtualAddress, size_bytes: usize) -> bool {
        let a_start = self.virtual_address().as_usize();
        let a_end = self.end_virtual_address().as_usize();

        let b_start = va.as_usize();
        let b_end = unsafe { va.offset(size_bytes).as_usize() };

        a_start < b_end && a_end > b_start
    }
}

impl MemoryRange for LogicalMemoryRange {
    fn virtual_address(&self) -> VirtualAddress {
        self.la.into_virtual()
    }

    fn size_bytes(&self) -> usize {
        self.size_bytes
    }
}

impl MemoryRange for VirtualMemoryRange {
    fn virtual_address(&self) -> VirtualAddress {
        self.va
    }

    fn size_bytes(&self) -> usize {
        self.size_bytes
    }
}

impl MemoryRange for GenericMemoryRange {
    fn virtual_address(&self) -> VirtualAddress {
        match self {
            GenericMemoryRange::Logical(range) => range.virtual_address(),
            GenericMemoryRange::Virtual(range) => range.virtual_address(),
        }
    }

    fn size_bytes(&self) -> usize {
        match self {
            GenericMemoryRange::Logical(range) => range.size_bytes(),
            GenericMemoryRange::Virtual(range) => range.size_bytes(),
        }
    }
}

pub(super) struct KernelAddressSpace {
    // FIXME(jalv): Using vec here is most likely not a good idea for performance reasons.
    // Find a better alternative with better insersion/removal/lookup performance
    virtual_ranges: Vec<VirtualMemoryRange>,
    logical_ranges: Vec<LogicalMemoryRange>,
}

impl KernelAddressSpace {
    pub const fn new() -> Self {
        Self {
            virtual_ranges: vec![],
            logical_ranges: vec![],
        }
    }

    fn check_overlaps(&self, va: VirtualAddress, size_bytes: usize) -> Result<(), Error> {
        if let Some(range) = self
            .logical_ranges
            .iter()
            .find(|range| range.overlaps(va, size_bytes))
        {
            return Err(Error::MemoryRangeOverlaps(range.name.clone()));
        }

        Ok(())
    }

    fn find_by_name<'a>(&'a mut self, name: &str) -> Result<&'a dyn MemoryRange, Error> {
        if let Some(range) = self
            .logical_ranges
            .iter_mut()
            .find(|range| range.name == name)
        {
            return Ok(range);
        }

        if let Some(range) = self
            .virtual_ranges
            .iter_mut()
            .find(|range| range.name == name)
        {
            return Ok(range);
        }

        Err(Error::MemoryRangeNotFound(
            String::from_str(name).map_err(|_| Error::NameTooLong)?,
        ))
    }

    pub fn add_logical_range<'a>(
        &'a mut self,
        name: &str,
        la: LogicalAddress,
        size_bytes: usize,
        attributes: Attributes,
        permissions: Permissions,
    ) -> Result<&'a LogicalMemoryRange, Error> {
        println!(
            "Adding logical range `{}` at {}, size 0x{:x}, permissions {:?}",
            name, la, size_bytes, permissions
        );

        self.check_overlaps(la.into_virtual(), size_bytes)?;

        if self.find_by_name(name).is_ok() {
            return Err(Error::MemoryRangeAlreadyExists(name.into()));
        }

        let memory_range = LogicalMemoryRange {
            la,
            name: String::from_str(name).map_err(|_| Error::NameTooLong)?,
            size_bytes,
            attributes,
            permissions,
        };
        self.logical_ranges.push(memory_range);

        Ok(self.logical_ranges.last().as_ref().unwrap())
    }

    pub fn _add_virtual_range(
        &mut self,
        name: &str,
        va: VirtualAddress,
        size_bytes: usize,
        attributes: Attributes,
        permissions: Permissions,
        pages: Vec<PhysicalPage>,
    ) -> Result<(), Error> {
        self.check_overlaps(va, size_bytes)?;

        if self.find_by_name(name).is_ok() {
            return Err(Error::MemoryRangeAlreadyExists(name.into()));
        }

        let memory_range = VirtualMemoryRange {
            va,
            name: String::from_str(name).map_err(|_| Error::NameTooLong)?,
            size_bytes,
            _attributes: attributes,
            _permissions: permissions,
            _pages: pages,
        };
        self.virtual_ranges.push(memory_range);

        Ok(())
    }

    pub fn remove_range_by_name(&mut self, name: &str) -> Result<GenericMemoryRange, Error> {
        if let Some((index, _range)) = self
            .logical_ranges
            .iter_mut()
            .enumerate()
            .find(|(_idx, range)| range.name == name)
        {
            let range = self.logical_ranges.remove(index);
            return Ok(range.into());
        }

        if let Some((index, _range)) = self
            .virtual_ranges
            .iter_mut()
            .enumerate()
            .find(|(_idx, range)| range.name == name)
        {
            let range = self.virtual_ranges.remove(index);
            return Ok(range.into());
        }

        Err(Error::MemoryRangeNotFound(
            String::from_str(name).map_err(|_| Error::NameTooLong)?,
        ))
    }
}
