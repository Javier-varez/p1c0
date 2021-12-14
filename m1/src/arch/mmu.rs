extern crate alloc;

use alloc::boxed::Box;
use core::ops::{Deref, DerefMut};
use cortex_a::{
    asm::barrier,
    registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1, TTBR0_EL1, TTBR1_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

#[cfg(not(test))]
use crate::println;

const VA_MASK: u64 = (1 << 48) - (1 << 14);
const PA_MASK: u64 = (1 << 48) - (1 << 14);
const PAGE_SIZE: usize = 1 << 14;

static mut MMU: MemoryManagementUnit = MemoryManagementUnit::new();

#[derive(Debug)]
pub enum Error {
    OverlapsExistingMapping(VirtualAddress, TranslationLevel),
    UnalignedAddress,
}

#[derive(Clone, Copy, Debug)]
pub enum Attributes {
    Normal = 0,
    DevicenGnRnE = 1,
    DevicenGnRE = 2,
}

impl Attributes {
    const MAIR_ATTR_OFFSET: usize = 2;
    fn mair_index(&self) -> u64 {
        ((*self as u64) & 0x7) << Self::MAIR_ATTR_OFFSET
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Permissions {
    RWX = 0,
    RW = 1,
    RX = 2,
    RO = 3,
}

impl Permissions {
    fn ap_bits(&self) -> u64 {
        match *self {
            Permissions::RWX | Permissions::RW => 0b00 << 6,
            Permissions::RX | Permissions::RO => 0b10 << 6,
        }
    }

    fn nx_bits(&self) -> u64 {
        match *self {
            Permissions::RWX | Permissions::RX => 0,
            Permissions::RW | Permissions::RO => 0x3 << 53, // UXN and PXN bits
        }
    }

    fn bits(&self) -> u64 {
        self.ap_bits() | self.nx_bits()
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct VirtualAddress(*const u8);

impl VirtualAddress {
    pub fn new(addr: *const u8) -> Result<Self, Error> {
        let addr_usize = addr as usize;
        if (addr_usize & (PAGE_SIZE - 1)) != 0 {
            return Err(Error::UnalignedAddress);
        }
        Ok(Self(addr))
    }
    pub unsafe fn offset(&self, offset: usize) -> Self {
        Self(self.0.add(offset))
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct PhysicalAddress(*const u8);

impl PhysicalAddress {
    pub fn new(addr: *const u8) -> Result<Self, Error> {
        let addr_usize = addr as usize;
        if (addr_usize & (PAGE_SIZE - 1)) != 0 {
            return Err(Error::UnalignedAddress);
        }
        Ok(Self(addr))
    }
    pub unsafe fn offset(&self, offset: usize) -> Self {
        Self(self.0.add(offset))
    }
}

#[derive(Eq, PartialEq, Debug)]
enum DescriptorType {
    Invalid,
    Page,
    Block,
    Table,
}

#[derive(Clone, Debug)]
#[repr(C)]
struct DescriptorEntry(u64);

impl DescriptorEntry {
    const VALID_BIT: u64 = 1 << 0;
    const TABLE_BIT: u64 = 1 << 1;
    const ACCESS_FLAG: u64 = 1 << 10;
    const PAGE_BIT: u64 = 1 << 55;
    const SHAREABILITY: u64 = 0b10 << 8; // Output shareable

    const fn new_invalid() -> Self {
        Self(0)
    }

    fn new_table_desc(level: TranslationLevel) -> Self {
        let table = Box::new(LevelTable::new(level));
        let table_addr = Box::leak(table) as *mut _;
        Self(Self::VALID_BIT | Self::TABLE_BIT | (table_addr as u64 & PA_MASK))
    }

    fn new_block_desc(
        physical_addr: PhysicalAddress,
        attributes: Attributes,
        permissions: Permissions,
    ) -> Self {
        Self(
            Self::VALID_BIT
                | Self::ACCESS_FLAG
                | (physical_addr.0 as u64 & PA_MASK)
                | attributes.mair_index()
                | Self::SHAREABILITY
                | permissions.bits(),
        )
    }

    fn new_page_desc(
        physical_addr: PhysicalAddress,
        attributes: Attributes,
        permissions: Permissions,
    ) -> Self {
        Self(
            Self::VALID_BIT
                | Self::TABLE_BIT
                | Self::PAGE_BIT
                | Self::ACCESS_FLAG
                | (physical_addr.0 as u64 & PA_MASK)
                | attributes.mair_index()
                | Self::SHAREABILITY
                | permissions.bits(),
        )
    }

    fn get_table(&mut self) -> Option<&mut LevelTable> {
        match self.ty() {
            DescriptorType::Table => {
                let table_ptr = (self.0 & VA_MASK) as *mut _;
                Some(unsafe { &mut *table_ptr })
            }
            _ => None,
        }
    }

    fn ty(&self) -> DescriptorType {
        if (self.0 & Self::VALID_BIT) == 0 {
            DescriptorType::Invalid
        } else if (self.0 & Self::TABLE_BIT) == 0 {
            DescriptorType::Block
        } else if (self.0 & Self::PAGE_BIT) == 0 {
            DescriptorType::Table
        } else {
            DescriptorType::Page
        }
    }

    fn pa(&self) -> Option<PhysicalAddress> {
        match self.ty() {
            DescriptorType::Page | DescriptorType::Block => {
                Some(PhysicalAddress((self.0 & PA_MASK) as *const u8))
            }
            _ => None,
        }
    }
}

impl Drop for DescriptorEntry {
    fn drop(&mut self) {
        if let Some(table) = self.get_table() {
            // Free table
            let table_box = unsafe { Box::from_raw(table as *mut _) };
            drop(table_box);
        }
    }
}

const INVALID_DESCRIPTOR: DescriptorEntry = DescriptorEntry::new_invalid();

/// Translation granule is hardcoded to 16KB
/// size of L2 memory region is 32MB
/// size of L1 memory region is 64GB
/// size of L0 memory region is 128TB
/// Total addressable memory is 256TB
///
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TranslationLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}

impl TranslationLevel {
    fn is_last(&self) -> bool {
        *self == TranslationLevel::Level3
    }

    fn supports_block_descriptors(&self) -> bool {
        matches!(*self, TranslationLevel::Level2 | TranslationLevel::Level3)
    }

    fn address_range_size(&self) -> usize {
        match *self {
            TranslationLevel::Level0 => 1 << 48,
            TranslationLevel::Level1 => 1 << 47,
            TranslationLevel::Level2 => 1 << 36,
            TranslationLevel::Level3 => 1 << 25,
        }
    }

    fn entry_size(&self) -> usize {
        1 << self.offset()
    }

    fn va_mask(&self) -> usize {
        self.address_range_size() - self.entry_size()
    }

    fn offset(&self) -> usize {
        match *self {
            TranslationLevel::Level0 => 47,
            TranslationLevel::Level1 => 36,
            TranslationLevel::Level2 => 25,
            TranslationLevel::Level3 => 14,
        }
    }

    fn table_index_for_addr(&self, va: VirtualAddress) -> usize {
        let va = va.0 as usize;
        (va & self.va_mask()) >> self.offset()
    }

    fn is_address_aligned(&self, va: VirtualAddress) -> bool {
        let va = va.0 as usize;
        (va % self.entry_size()) == 0
    }

    fn next(&self) -> Self {
        match *self {
            TranslationLevel::Level0 => TranslationLevel::Level1,
            TranslationLevel::Level1 => TranslationLevel::Level2,
            TranslationLevel::Level2 => TranslationLevel::Level3,
            TranslationLevel::Level3 => TranslationLevel::Level3,
        }
    }
}

/// Levels must be aligned at least at 16KB according to the translation granule.
/// In addition, each level must be 11 bits, that's why we have 2048 entries in a level table
#[repr(C, align(0x4000))]
struct LevelTable {
    table: [DescriptorEntry; 2048],
    level: TranslationLevel,
}

impl Deref for LevelTable {
    type Target = [DescriptorEntry];
    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

impl DerefMut for LevelTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

impl LevelTable {
    const fn new(level: TranslationLevel) -> Self {
        Self {
            table: [INVALID_DESCRIPTOR; 2048],
            level,
        }
    }
}

pub struct MemoryManagementUnit {
    initialized: bool,
    level0: LevelTable,
}

impl MemoryManagementUnit {
    const fn new() -> Self {
        Self {
            initialized: false,
            level0: LevelTable::new(TranslationLevel::Level0),
        }
    }

    fn add_default_mappings(&mut self) {
        let ram_base = 0x10000000000 as *const u8;
        let ram_size = 0x800000000;
        self.map_region(
            VirtualAddress::new(ram_base).expect("Address is aligned to page size"),
            PhysicalAddress::new(ram_base).expect("Address is aligned to page size"),
            ram_size,
            Attributes::Normal,
            Permissions::RWX,
        )
        .expect("No other mapping overlaps");

        let mmio_region_base = 0x0000000200000000 as *const u8;
        let mmio_region_size = 0x0000000400000000;
        self.map_region(
            VirtualAddress::new(mmio_region_base).expect("Address is aligned to page size"),
            PhysicalAddress::new(mmio_region_base).expect("Address is aligned to page size"),
            mmio_region_size,
            Attributes::DevicenGnRE,
            Permissions::RWX,
        )
        .expect("No other mapping overlaps");

        let mmio_region_base = 0x0000000580000000 as *const u8;
        let mmio_region_size = 0x0000000180000000;
        self.map_region(
            VirtualAddress::new(mmio_region_base).expect("Address is aligned to page size"),
            PhysicalAddress::new(mmio_region_base).expect("Address is aligned to page size"),
            mmio_region_size,
            Attributes::DevicenGnRE,
            Permissions::RWX,
        )
        .expect("No other mapping overlaps");

        let mmio_region_base = 0x0000000700000000 as *const u8;
        let mmio_region_size = 0x0000000F80000000;
        self.map_region(
            VirtualAddress::new(mmio_region_base).expect("Address is aligned to page size"),
            PhysicalAddress::new(mmio_region_base).expect("Address is aligned to page size"),
            mmio_region_size,
            Attributes::DevicenGnRE,
            Permissions::RWX,
        )
        .expect("No other mapping overlaps");
    }

    pub fn init_and_enable(&mut self) {
        if self.initialized {
            return;
        }
        self.add_default_mappings();
        self.enable();
        self.initialized = true;
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn enable(&self) {
        MAIR_EL1.write(
            MAIR_EL1::Attr0_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc
                + MAIR_EL1::Attr0_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc
                + MAIR_EL1::Attr1_Device::nonGathering_nonReordering_noEarlyWriteAck
                + MAIR_EL1::Attr2_Device::nonGathering_nonReordering_EarlyWriteAck,
        );

        TCR_EL1.write(
            TCR_EL1::IPS::Bits_48
                + TCR_EL1::TG1::KiB_16
                + TCR_EL1::TG0::KiB_16
                + TCR_EL1::SH1::Inner
                + TCR_EL1::SH0::Inner
                + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::ORGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::IRGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                + TCR_EL1::T0SZ.val(16)
                + TCR_EL1::T1SZ.val(16),
        );

        TTBR0_EL1.set_baddr(self.level0.table.as_ptr() as u64);
        TTBR1_EL1.set_baddr(self.level0.table.as_ptr() as u64);

        unsafe {
            barrier::dsb(barrier::ISHST);
            barrier::isb(barrier::SY);
        }

        SCTLR_EL1.modify(SCTLR_EL1::M::Enable + SCTLR_EL1::C::Cacheable + SCTLR_EL1::I::Cacheable);

        unsafe {
            barrier::isb(barrier::SY);
        }

        if matches!(
            SCTLR_EL1.read_as_enum(SCTLR_EL1::M),
            Some(SCTLR_EL1::M::Value::Enable)
        ) {
            println!("MMU enabled");
        } else {
            println!("Error enabling MMU");
        }
    }

    fn map_region_internal(
        mut va: VirtualAddress,
        mut pa: PhysicalAddress,
        size: usize,
        attributes: Attributes,
        permissions: Permissions,
        translation_table: &mut LevelTable,
    ) -> Result<(), Error> {
        let level = translation_table.level;
        let entry_size = level.entry_size();

        let mut remaining_size = size;
        while remaining_size != 0 {
            let index = translation_table.level.table_index_for_addr(va);
            let aligned = translation_table.level.is_address_aligned(va);
            let descriptor_entry = &mut translation_table[index];

            let chunk_size = if !aligned {
                let next_level = level.next();
                let rem_entry_size =
                    entry_size - next_level.table_index_for_addr(va) * next_level.entry_size();
                core::cmp::min(rem_entry_size, remaining_size)
            } else {
                core::cmp::min(entry_size, remaining_size)
            };

            if aligned
                && (chunk_size == entry_size)
                && (level.supports_block_descriptors() || level.is_last())
            {
                // We could allocate a block or page descriptor here. Else we would need a next
                // level table
                match descriptor_entry.ty() {
                    DescriptorType::Invalid => {
                        *descriptor_entry = if level.is_last() {
                            DescriptorEntry::new_page_desc(pa, attributes, permissions)
                        } else {
                            DescriptorEntry::new_block_desc(pa, attributes, permissions)
                        };
                    }
                    DescriptorType::Table => {
                        // TODO(javier-varez): Handle consolidation of mappings
                        return Err(Error::OverlapsExistingMapping(va, level));
                    }
                    DescriptorType::Page | DescriptorType::Block
                        if descriptor_entry.pa().expect("Desc is a page/block") != pa =>
                    {
                        return Err(Error::OverlapsExistingMapping(va, level));
                    }
                    _ => {}
                }
            } else {
                match descriptor_entry.ty() {
                    DescriptorType::Block | DescriptorType::Page => {
                        // Need to have a table and some granularity inside
                        return Err(Error::OverlapsExistingMapping(va, level));
                    }
                    DescriptorType::Invalid => {
                        *descriptor_entry = DescriptorEntry::new_table_desc(level.next());
                    }
                    _ => {}
                }

                Self::map_region_internal(
                    va,
                    pa,
                    chunk_size,
                    attributes,
                    permissions,
                    descriptor_entry.get_table().expect("Is a table"),
                )?;
            }

            unsafe {
                pa = pa.offset(chunk_size);
                va = va.offset(chunk_size);
            }

            remaining_size = remaining_size.saturating_sub(chunk_size);
        }
        Ok(())
    }

    pub fn map_region(
        &mut self,
        va: VirtualAddress,
        pa: PhysicalAddress,
        mut size: usize,
        attributes: Attributes,
        permissions: Permissions,
    ) -> Result<(), Error> {
        let translation_table = &mut self.level0;

        println!(
            "Adding mapping from {:?} to {:?}, size 0x{:x}",
            va, pa, size
        );

        // Size needs to be aligned to page size
        if (size % PAGE_SIZE) != 0 {
            size = size + PAGE_SIZE - (size % PAGE_SIZE);
        }

        Self::map_region_internal(va, pa, size, attributes, permissions, translation_table)
    }
}

pub fn initialize() {
    unsafe { MMU.init_and_enable() };
}

pub fn is_initialized() -> bool {
    unsafe { MMU.is_initialized() }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn single_page_mapping() {
        let mut mmu = MemoryManagementUnit::new();

        assert!(matches!(mmu.level0[0].ty(), DescriptorType::Invalid));

        let from = VirtualAddress::new(0x012345678000 as *const u8).unwrap();
        let to = PhysicalAddress::new(0x012345678000 as *const u8).unwrap();
        let size = 1 << 14;
        mmu.map_region(from, to, size, Attributes::Normal, Permissions::RWX)
            .expect("Adding region was successful");

        let level0 = &mut mmu.level0;
        assert_eq!(level0.level, TranslationLevel::Level0);
        assert!(matches!(level0[0].ty(), DescriptorType::Table));
        assert!(matches!(level0[1].ty(), DescriptorType::Invalid));

        let level1 = level0[0].get_table().expect("Is a table");
        assert_eq!(level1.level, TranslationLevel::Level1);
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x12 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x12].get_table().expect("Is a table");
        assert_eq!(level2.level, TranslationLevel::Level2);

        for (idx, desc) in level2.table.iter().enumerate() {
            if idx == 0x1a2 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level3 = level2[0x1a2].get_table().expect("Is a table");
        assert_eq!(level3.level, TranslationLevel::Level3);

        for (idx, desc) in level3.table.iter().enumerate() {
            if idx == 0x59e {
                assert!(matches!(desc.ty(), DescriptorType::Page));
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }
    }

    #[test]
    fn single_block_mapping() {
        let mut mmu = MemoryManagementUnit::new();

        assert!(matches!(mmu.level0[0].ty(), DescriptorType::Invalid));

        let from = VirtualAddress::new(0x12344000000 as *const u8).unwrap();
        let to = PhysicalAddress::new(0x12344000000 as *const u8).unwrap();
        let size = 1 << 25;
        mmu.map_region(from, to, size, Attributes::Normal, Permissions::RWX)
            .expect("Adding region was successful");

        let level0 = &mut mmu.level0;
        assert_eq!(level0.level, TranslationLevel::Level0);
        assert!(matches!(level0[0].ty(), DescriptorType::Table));
        assert!(matches!(level0[1].ty(), DescriptorType::Invalid));

        let level1 = level0[0].get_table().expect("Is a table");
        assert_eq!(level1.level, TranslationLevel::Level1);
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x12 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x12].get_table().expect("Is a table");
        assert_eq!(level2.level, TranslationLevel::Level2);

        for (idx, desc) in level2.table.iter().enumerate() {
            if idx == 0x1a2 {
                assert!(matches!(desc.ty(), DescriptorType::Block));
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }
    }

    #[test]
    fn large_aligned_block_mapping() {
        let mut mmu = MemoryManagementUnit::new();

        assert!(matches!(mmu.level0[0].ty(), DescriptorType::Invalid));

        let block_size = 1 << 25;
        let page_size = 1 << 14;

        let from = VirtualAddress::new(0x12344000000 as *const u8).unwrap();
        let to = PhysicalAddress::new(0x12344000000 as *const u8).unwrap();
        let size = block_size + page_size * 4;
        mmu.map_region(from, to, size, Attributes::Normal, Permissions::RWX)
            .expect("Adding region was successful");

        let level0 = &mut mmu.level0;
        assert_eq!(level0.level, TranslationLevel::Level0);
        assert!(matches!(level0[0].ty(), DescriptorType::Table));
        assert!(matches!(level0[1].ty(), DescriptorType::Invalid));

        let level1 = level0[0].get_table().expect("Is a table");
        assert_eq!(level1.level, TranslationLevel::Level1);
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x12 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x12].get_table().expect("Is a table");
        assert_eq!(level2.level, TranslationLevel::Level2);

        for (idx, desc) in level2.table.iter().enumerate() {
            if idx == 0x1a2 {
                assert!(matches!(desc.ty(), DescriptorType::Block));
                assert_eq!(desc.pa(), Some(to));
            } else if idx == 0x1a3 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level3 = level2[0x1a3].get_table().expect("Is a table");
        assert_eq!(level3.level, TranslationLevel::Level3);
        for (idx, desc) in level3.table.iter().enumerate() {
            if idx < 4 {
                assert!(matches!(desc.ty(), DescriptorType::Page));
                let to_usize = to.0 as usize + block_size + page_size * idx;
                let to = PhysicalAddress::new(to_usize as *const _).expect("Address is aligned");
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }
    }

    #[test]
    fn large_unaligned_block_mapping() {
        let mut mmu = MemoryManagementUnit::new();

        assert!(matches!(mmu.level0[0].ty(), DescriptorType::Invalid));

        let block_size = 1 << 25;
        let page_size = 1 << 14;
        let va = 0x12344000000 + page_size * 2044;

        let from = VirtualAddress::new(va as *const u8).unwrap();
        let to = PhysicalAddress::new(0x12344000000 as *const u8).unwrap();
        let size = block_size + page_size * 4;
        mmu.map_region(from, to, size, Attributes::Normal, Permissions::RWX)
            .expect("Adding region was successful");

        let level0 = &mut mmu.level0;
        assert_eq!(level0.level, TranslationLevel::Level0);
        assert!(matches!(level0[0].ty(), DescriptorType::Table));
        assert!(matches!(level0[1].ty(), DescriptorType::Invalid));

        let level1 = level0[0].get_table().expect("Is a table");
        assert_eq!(level1.level, TranslationLevel::Level1);
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x12 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x12].get_table().expect("Is a table");
        assert_eq!(level2.level, TranslationLevel::Level2);

        for (idx, desc) in level2.table.iter().enumerate() {
            if idx == 0x1a2 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else if idx == 0x1a3 {
                assert!(matches!(desc.ty(), DescriptorType::Block));
                let to_usize = to.0 as usize + page_size * 4;
                let to = PhysicalAddress::new(to_usize as *const _).expect("Address is aligned");
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level3 = level2[0x1a2].get_table().expect("Is a table");
        assert_eq!(level3.level, TranslationLevel::Level3);

        for (idx, desc) in level3.table.iter().enumerate() {
            if idx >= 2044 {
                assert!(matches!(desc.ty(), DescriptorType::Page));
                let to_usize = to.0 as usize + page_size * (idx - 2044);
                let to = PhysicalAddress::new(to_usize as *const _).expect("Address is aligned");
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }
    }

    #[test]
    fn real_mapping() {
        let mut mmu = MemoryManagementUnit::new();
        mmu.add_default_mappings();

        let level0 = &mut mmu.level0;
        assert_eq!(level0.level, TranslationLevel::Level0);
        assert!(matches!(level0[0].ty(), DescriptorType::Table));
        assert!(matches!(level0[1].ty(), DescriptorType::Invalid));

        let level1 = level0[0].get_table().expect("Is a table");
        assert_eq!(level1.level, TranslationLevel::Level1);
        for (idx, desc) in level1.table.iter().enumerate() {
            println!("level 1: {}", idx);
            if idx == 0x10 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            }
        }

        let level2 = level1[0x10].get_table().expect("Is a table");
        assert_eq!(level2.level, TranslationLevel::Level2);

        for (idx, desc) in level2.table.iter().enumerate() {
            if idx < 1024 {
                assert!(matches!(desc.ty(), DescriptorType::Block));
                let block_size = 1 << 25;
                let to_usize = 0x10000000000 as usize + block_size * idx;
                let to = PhysicalAddress::new(to_usize as *const _).expect("Address is aligned");
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }
    }
}
