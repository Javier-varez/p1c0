use super::{
    address::{Address, LogicalAddress, PhysicalAddress, VirtualAddress},
    map::{FASTMAP_PAGE, MMIO_BASE, MMIO_SIZE},
    num_pages_from_bytes,
    physical_page_allocator::PhysicalMemoryRegion,
    Attributes, GlobalPermissions, Permissions,
};
use crate::{
    arch::mmu::{self, LevelTable, PAGE_SIZE},
    prelude::*,
};

use core::str::FromStr;

use heapless::String;

const MAX_NAME_LENGTH: usize = 32;

#[derive(Clone, Debug)]
pub enum Error {
    ArchSpecificError(mmu::Error),
    MemoryRangeNotFound(String<MAX_NAME_LENGTH>),
    MemoryRangeAlreadyExists(String<MAX_NAME_LENGTH>),
    MemoryRangeOverlaps(String<MAX_NAME_LENGTH>),
    NameTooLong,
    InvalidAddress,
}

impl From<mmu::Error> for Error {
    fn from(e: mmu::Error) -> Self {
        Error::ArchSpecificError(e)
    }
}

pub(super) struct VirtualMemoryRange {
    pub va: VirtualAddress,
    pub size_bytes: usize,
    pub name: String<MAX_NAME_LENGTH>,
    pub _attributes: Attributes,
    pub _permissions: GlobalPermissions,
    pub _pmr: PhysicalMemoryRegion,
    // We can later add operations based on backed descriptors here
}

pub(super) struct LogicalMemoryRange {
    pub la: LogicalAddress,
    pub size_bytes: usize,
    pub name: String<32>,
    pub attributes: Attributes,
    pub permissions: Permissions,
    pub _physical_region: Option<PhysicalMemoryRegion>,
}

pub(super) struct MMIORange {
    pub va: VirtualAddress,
    pub pa: PhysicalAddress,
    pub size_bytes: usize,
    pub name: String<32>,
}

pub(super) enum GenericMemoryRange {
    Logical(LogicalMemoryRange),
    Virtual(VirtualMemoryRange),
    Mmio(MMIORange),
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

impl From<MMIORange> for GenericMemoryRange {
    fn from(mmio_range: MMIORange) -> Self {
        Self::Mmio(mmio_range)
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

impl MemoryRange for MMIORange {
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
            GenericMemoryRange::Mmio(range) => range.virtual_address(),
        }
    }

    fn size_bytes(&self) -> usize {
        match self {
            GenericMemoryRange::Logical(range) => range.size_bytes(),
            GenericMemoryRange::Virtual(range) => range.size_bytes(),
            GenericMemoryRange::Mmio(range) => range.size_bytes(),
        }
    }
}

pub(super) struct KernelAddressSpace {
    // FIXME(javier-varez): Using vec here is most likely not a good idea for performance reasons.
    // Find a better alternative with better insertion/removal/lookup performance
    high_address_table: LevelTable,
    low_address_table: LevelTable,
    virtual_ranges: Vec<VirtualMemoryRange>,
    logical_ranges: Vec<LogicalMemoryRange>,
    mmio_ranges: Vec<MMIORange>,
    mmio_offset: usize,
}

impl KernelAddressSpace {
    pub const fn new() -> Self {
        Self {
            high_address_table: LevelTable::new(),
            low_address_table: LevelTable::new(),
            virtual_ranges: vec![],
            logical_ranges: vec![],
            mmio_ranges: vec![],
            mmio_offset: 0,
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

    pub fn add_logical_range(
        &mut self,
        name: &str,
        la: LogicalAddress,
        size_bytes: usize,
        attributes: Attributes,
        permissions: Permissions,
        physical_region: Option<PhysicalMemoryRegion>,
    ) -> Result<&LogicalMemoryRange, Error> {
        log_info!(
            "Adding logical range `{}` at {}, size 0x{:x}, permissions {:?}",
            name,
            la,
            size_bytes,
            permissions
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
            _physical_region: physical_region,
        };
        self.logical_ranges.push(memory_range);

        Ok(self.logical_ranges.last().as_ref().unwrap())
    }

    pub fn allocate_io_range(
        &mut self,
        name: &str,
        pa: PhysicalAddress,
        size_bytes: usize,
    ) -> Result<VirtualAddress, Error> {
        let num_pages = num_pages_from_bytes(size_bytes);

        if self.mmio_offset + num_pages * PAGE_SIZE > MMIO_SIZE {
            panic!("MMIO Range is exhausted!");
        }

        let offset = self.mmio_offset;
        let va = unsafe { MMIO_BASE.offset(offset) };

        self.mmio_offset += num_pages * PAGE_SIZE;

        let range = MMIORange {
            va,
            pa,
            name: String::from_str(name).map_err(|_| Error::NameTooLong)?,
            size_bytes,
        };
        self.mmio_ranges.push(range);

        log_info!(
            "Adding io range `{}` at {}, size 0x{:x}",
            name,
            va,
            size_bytes
        );

        Ok(va)
    }

    pub fn remove_range_by_name(
        &mut self,
        name: &str,
    ) -> Result<(&mut LevelTable, GenericMemoryRange), Error> {
        if let Some((index, _range)) = self
            .logical_ranges
            .iter_mut()
            .enumerate()
            .find(|(_idx, range)| range.name == name)
        {
            let range = self.logical_ranges.remove(index);
            return Ok((&mut self.high_address_table, range.into()));
        }

        if let Some((index, _range)) = self
            .virtual_ranges
            .iter_mut()
            .enumerate()
            .find(|(_idx, range)| range.name == name)
        {
            let range = self.virtual_ranges.remove(index);
            return Ok((&mut self.high_address_table, range.into()));
        }

        if let Some((index, _range)) = self
            .mmio_ranges
            .iter_mut()
            .enumerate()
            .find(|(_idx, range)| range.name == name)
        {
            let range = self.mmio_ranges.remove(index);
            return Ok((&mut self.high_address_table, range.into()));
        }

        Err(Error::MemoryRangeNotFound(
            String::from_str(name).map_err(|_| Error::NameTooLong)?,
        ))
    }

    pub(super) fn fast_page_map(
        &mut self,
        pa: PhysicalAddress,
        permissions: GlobalPermissions,
        attributes: Attributes,
    ) -> Result<(), Error> {
        self.high_address_table
            .map_region(FASTMAP_PAGE, pa, PAGE_SIZE, attributes, permissions)?;

        // Ensure that the TLB discards any entries for the fast map page
        mmu::flush_tlb_page(FASTMAP_PAGE);

        Ok(())
    }

    pub(super) fn fast_page_unmap(&mut self) -> Result<(), Error> {
        self.high_address_table
            .unmap_region(FASTMAP_PAGE, PAGE_SIZE)?;

        // Ensure that the TLB discards any entries for the fast map page
        mmu::flush_tlb_page(FASTMAP_PAGE);

        Ok(())
    }

    pub(super) fn high_table(&mut self) -> &mut LevelTable {
        &mut self.high_address_table
    }

    pub(super) fn low_table(&mut self) -> &mut LevelTable {
        &mut self.low_address_table
    }

    pub(super) fn tables(&mut self) -> (&mut LevelTable, &mut LevelTable) {
        (&mut self.high_address_table, &mut self.low_address_table)
    }

    pub(super) fn resolve_address(&self, va: VirtualAddress) -> Result<PhysicalAddress, Error> {
        // Resolving a logical address is easy, so check if the VA is actually logical
        if let Ok(la) = va.try_into_logical() {
            return Ok(la.into_physical());
        }

        // Out of luck, we need to check the MMIO ranges now
        for range in &self.mmio_ranges {
            if range.overlaps(va, 1) {
                // This range matches the address.
                let offset = va.offset_from(range.va);
                assert!(offset >= 0);

                return Ok(unsafe { range.pa.offset(offset as usize) });
            }
        }

        // This doesn't seem to match any ranges
        Err(Error::InvalidAddress)
    }
}

pub struct ProcessAddressSpace {
    address_table: Box<LevelTable>,
    // FIXME(javier-varez): Using vec here is most likely not a good idea for performance reasons.
    // Find a better alternative with better insertion/removal/lookup performance
    memory_ranges: Vec<VirtualMemoryRange>,
}

impl Default for ProcessAddressSpace {
    fn default() -> Self {
        Self {
            address_table: Box::new(LevelTable::new()),
            memory_ranges: vec![],
        }
    }
}

impl ProcessAddressSpace {
    pub fn new() -> Self {
        Self::default()
    }

    fn check_overlaps(&self, va: VirtualAddress, size_bytes: usize) -> Result<(), Error> {
        if let Some(range) = self
            .memory_ranges
            .iter()
            .find(|range| range.overlaps(va, size_bytes))
        {
            return Err(Error::MemoryRangeOverlaps(range.name.clone()));
        }

        Ok(())
    }

    fn find_by_name<'a>(&'a mut self, name: &str) -> Result<&'a dyn MemoryRange, Error> {
        if let Some(range) = self
            .memory_ranges
            .iter_mut()
            .find(|range| range.name == name)
        {
            return Ok(range);
        }

        Err(Error::MemoryRangeNotFound(
            String::from_str(name).map_err(|_| Error::NameTooLong)?,
        ))
    }

    fn add_virtual_range(
        &mut self,
        name: &str,
        va: VirtualAddress,
        pmr: PhysicalMemoryRegion,
        size_bytes: usize,
        attributes: Attributes,
        permissions: GlobalPermissions,
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
            _pmr: pmr,
        };
        self.memory_ranges.push(memory_range);

        Ok(())
    }

    pub(crate) fn address_table(&mut self) -> &mut LevelTable {
        &mut self.address_table
    }

    pub fn map_section(
        &mut self,
        name: &str,
        va: VirtualAddress,
        pmr: PhysicalMemoryRegion,
        size_bytes: usize,
        permissions: GlobalPermissions,
    ) -> Result<(), Error> {
        let pa = pmr.base_address();
        self.address_table
            .map_region(va, pa, size_bytes, Attributes::Normal, permissions)
            .unwrap();
        self.add_virtual_range(name, va, pmr, size_bytes, Attributes::Normal, permissions)
    }
}
