extern crate alloc;

mod early_alloc;

use alloc::boxed::Box;
use core::alloc::Allocator;
use core::ops::{Deref, DerefMut};
use cortex_a::{
    asm::barrier,
    registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1, TTBR0_EL1, TTBR1_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use core::mem::MaybeUninit;
use early_alloc::{AllocRef, EarlyAllocator};

use crate::pa_to_kla;

#[cfg(not(test))]
use crate::println;

const VA_MASK: u64 = (1 << 48) - (1 << 14);
const PA_MASK: u64 = (1 << 48) - (1 << 14);
const PAGE_SIZE: usize = 1 << 14;

const EARLY_ALLOCATOR_SIZE: usize = 128 * 1024;
static EARLY_ALLOCATOR: EarlyAllocator<EARLY_ALLOCATOR_SIZE> = EarlyAllocator::new();
pub static mut MMU: MemoryManagementUnit = MemoryManagementUnit::new();

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

    /// # Safety
    ///   The user must guarantee that the resulting pointer is a valid VirtualAddress after this
    ///   operation. This means that it is within the limits of addressable virtual memory.
    #[must_use]
    pub unsafe fn offset(&self, offset: usize) -> Self {
        Self(self.0.add(offset))
    }

    fn is_high_address(&self) -> bool {
        let high_bits = self.0 as usize >> 48;
        if high_bits == 0xFFFF {
            true
        } else if high_bits == 0x0000 {
            false
        } else {
            panic!("Virtual address is invalid");
        }
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

    pub fn from_unaligned(addr: *const u8) -> Result<Self, Error> {
        let mut addr_usize = addr as usize;
        addr_usize &= !(PAGE_SIZE - 1);
        Ok(Self(addr_usize as *const u8))
    }

    /// # Safety
    ///   The user must guarantee that the resulting pointer is a valid PhysicalAddress after this
    ///   operation. This means that it is within the limits of addressable physical memory and
    ///   points to a valid physical address backed by some memory device (either memory mapped IO or
    ///   regular memory).
    #[must_use]
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
    const EARLY_BIT: u64 = 1 << 56;
    const SHAREABILITY: u64 = 0b10 << 8; // Output shareable

    const fn new_invalid() -> Self {
        Self(0)
    }

    fn new_table_desc() -> Self {
        let early = unsafe { !MMU.is_initialized() };
        let table_addr = if early {
            // FIXME: I'd love to use box here, but it seems to be triggering a compiler failure at
            // the time this code was written (Rust 1.59.0 nightly). Hopefully this gets fixed soon
            // and then we could use Box::new_in.
            let layout = core::alloc::Layout::new::<LevelTable>();
            let table = AllocRef::new(&EARLY_ALLOCATOR)
                .allocate(layout)
                .expect("We have enough early memory")
                .as_ptr() as *mut MaybeUninit<LevelTable>;
            unsafe { (*table).write(LevelTable::new()) };
            table as *mut LevelTable
        } else {
            // This gives a virtual memory address, we need to translate it to its physical
            // address for the table
            let table = Box::new(LevelTable::new());
            let addr = Box::leak(table) as *mut LevelTable;
            crate::kla_to_pa_mut(addr)
        };
        let early_bit = if early { Self::EARLY_BIT } else { 0 };
        Self(Self::VALID_BIT | Self::TABLE_BIT | (table_addr as u64 & PA_MASK) | early_bit)
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
                if unsafe { MMU.is_initialized() } {
                    let table_ptr = crate::pa_to_kla_mut(table_ptr);
                    Some(unsafe { &mut *table_ptr })
                } else {
                    Some(unsafe { &mut *table_ptr })
                }
            }
            _ => None,
        }
    }

    fn is_early_table(&self) -> bool {
        (self.ty() == DescriptorType::Table) && (self.0 & Self::EARLY_BIT) != 0
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
        let early_table = self.is_early_table();
        if let Some(table) = self.get_table() {
            if early_table {
                // We don't deallocate these
            } else {
                // These are physical addresses, we need to translate them to kernel logical
                // addresses, since that is what our kmalloc allocator works with.
                let table = table as *mut LevelTable;
                let table_box = unsafe { Box::from_raw(table) };
                drop(table_box);
            }
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
    const fn new() -> Self {
        Self {
            table: [INVALID_DESCRIPTOR; 2048],
        }
    }

    fn map_region(
        &mut self,
        mut va: VirtualAddress,
        mut pa: PhysicalAddress,
        size: usize,
        attributes: Attributes,
        permissions: Permissions,
        level: TranslationLevel,
    ) -> Result<(), Error> {
        let entry_size = level.entry_size();

        let mut remaining_size = size;
        while remaining_size != 0 {
            let index = level.table_index_for_addr(va);
            let aligned = level.is_address_aligned(va);
            let descriptor_entry = &mut self.table[index];

            let chunk_size = if !aligned {
                let next_level = level.next();
                let rem_entry_size =
                    entry_size - next_level.table_index_for_addr(va) * next_level.entry_size();
                core::cmp::min(rem_entry_size, remaining_size)
            } else {
                core::cmp::min(entry_size, remaining_size)
            };

            if matches!(
                descriptor_entry.ty(),
                DescriptorType::Block | DescriptorType::Page
            ) {
                if descriptor_entry.pa().expect("Desc is a page/block") != pa {
                    return Err(Error::OverlapsExistingMapping(va, level));
                } else {
                    // The mapping is already present
                    unsafe {
                        pa = pa.offset(chunk_size);
                        va = va.offset(chunk_size);
                    }

                    remaining_size = remaining_size.saturating_sub(chunk_size);
                    continue;
                };
            }

            if aligned
                && (chunk_size == entry_size)
                && (level.supports_block_descriptors() || level.is_last())
                && !matches!(descriptor_entry.ty(), DescriptorType::Table)
            {
                *descriptor_entry = if level.is_last() {
                    DescriptorEntry::new_page_desc(pa, attributes, permissions)
                } else {
                    DescriptorEntry::new_block_desc(pa, attributes, permissions)
                };
            } else {
                if matches!(descriptor_entry.ty(), DescriptorType::Invalid) {
                    *descriptor_entry = DescriptorEntry::new_table_desc();
                }

                descriptor_entry
                    .get_table()
                    .expect("Is a table")
                    .map_region(va, pa, chunk_size, attributes, permissions, level.next())?;
            }

            unsafe {
                pa = pa.offset(chunk_size);
                va = va.offset(chunk_size);
            }

            remaining_size = remaining_size.saturating_sub(chunk_size);
        }
        Ok(())
    }
}

pub struct MemoryManagementUnit {
    initialized: bool,
    high_table: LevelTable,
    low_table: LevelTable,
}

impl MemoryManagementUnit {
    const fn new() -> Self {
        Self {
            initialized: false,
            high_table: LevelTable::new(),
            low_table: LevelTable::new(),
        }
    }

    fn add_default_mappings(&mut self) {
        let adt = crate::adt::get_adt().unwrap();
        let chosen = adt.find_node("/chosen").expect("There is a chosen node");
        let dram_base = chosen
            .find_property("dram-base")
            .and_then(|prop| prop.usize_value().ok())
            .map(|addr| addr as *const u8)
            .expect("There is a dram base");
        let dram_size = chosen
            .find_property("dram-size")
            .and_then(|prop| prop.usize_value().ok())
            .expect("There is a dram base");

        // Add initial identity mapping. To be removed after relocation.
        self.map_region(
            VirtualAddress::new(dram_base).expect("Address is aligned to page size"),
            PhysicalAddress::new(dram_base).expect("Address is aligned to page size"),
            dram_size,
            Attributes::Normal,
            Permissions::RWX,
        )
        .expect("No other mapping overlaps");

        // Add secondary mapping at high_kernel_addr.
        self.map_region(
            VirtualAddress::new(pa_to_kla(dram_base)).expect("Address is aligned to page size"),
            PhysicalAddress::new(dram_base).expect("Address is aligned to page size"),
            dram_size,
            Attributes::Normal,
            Permissions::RWX,
        )
        .expect("No other mapping overlaps");

        // Map mmio ranges as defined in the ADT

        let root_address_cells = adt.find_node("/").and_then(|node| node.get_address_cells());
        let node = adt.find_node("/arm-io").expect("There is an arm-io");
        let range_iter = node.range_iter(root_address_cells);
        for range in range_iter {
            let mmio_region_base = range.get_parent_addr() as *const u8;
            let mmio_region_size = range.get_size();
            self.map_region(
                VirtualAddress::new(mmio_region_base).expect("Address is aligned to page size"),
                PhysicalAddress::new(mmio_region_base).expect("Address is aligned to page size"),
                mmio_region_size,
                Attributes::DevicenGnRnE,
                Permissions::RWX,
            )
            .expect("No other mapping overlaps");
        }
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

        TTBR0_EL1.set_baddr(self.low_table.table.as_ptr() as u64);
        TTBR1_EL1.set_baddr(self.high_table.table.as_ptr() as u64);

        unsafe {
            barrier::dsb(barrier::ISHST);
            barrier::isb(barrier::SY);
        }

        SCTLR_EL1.modify(SCTLR_EL1::M::Enable + SCTLR_EL1::C::Cacheable + SCTLR_EL1::I::Cacheable);

        unsafe {
            core::arch::asm!("dsb ishst");
            core::arch::asm!("tlbi vmalle1is");
            core::arch::asm!("dsb ish");
            core::arch::asm!("isb");
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

    pub fn map_region(
        &mut self,
        va: VirtualAddress,
        pa: PhysicalAddress,
        mut size: usize,
        attributes: Attributes,
        permissions: Permissions,
    ) -> Result<(), Error> {
        println!(
            "Adding mapping from {:?} to {:?}, size 0x{:x}",
            va, pa, size
        );

        // Size needs to be aligned to page size
        if (size % PAGE_SIZE) != 0 {
            size = size + PAGE_SIZE - (size % PAGE_SIZE);
        }

        let table = if va.is_high_address() {
            &mut self.high_table
        } else {
            &mut self.low_table
        };

        table.map_region(
            va,
            pa,
            size,
            attributes,
            permissions,
            TranslationLevel::Level0,
        )
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
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU.initialized = true };

        assert!(matches!(mmu.low_table[0].ty(), DescriptorType::Invalid));

        let from = VirtualAddress::new(0x012345678000 as *const u8).unwrap();
        let to = PhysicalAddress::new(0x012345678000 as *const u8).unwrap();
        let size = 1 << 14;
        mmu.map_region(from, to, size, Attributes::Normal, Permissions::RWX)
            .expect("Adding region was successful");

        let low_table = &mut mmu.low_table;
        assert!(matches!(low_table[0].ty(), DescriptorType::Table));
        assert!(matches!(low_table[1].ty(), DescriptorType::Invalid));

        let level1 = low_table[0].get_table().expect("Is a table");
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x12 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x12].get_table().expect("Is a table");

        for (idx, desc) in level2.table.iter().enumerate() {
            if idx == 0x1a2 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level3 = level2[0x1a2].get_table().expect("Is a table");

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
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU.initialized = true };

        assert!(matches!(mmu.low_table[0].ty(), DescriptorType::Invalid));

        let from = VirtualAddress::new(0x12344000000 as *const u8).unwrap();
        let to = PhysicalAddress::new(0x12344000000 as *const u8).unwrap();
        let size = 1 << 25;
        mmu.map_region(from, to, size, Attributes::Normal, Permissions::RWX)
            .expect("Adding region was successful");

        let low_table = &mut mmu.low_table;
        assert!(matches!(low_table[0].ty(), DescriptorType::Table));
        assert!(matches!(low_table[1].ty(), DescriptorType::Invalid));

        let level1 = low_table[0].get_table().expect("Is a table");
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x12 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x12].get_table().expect("Is a table");

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
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU.initialized = true };

        assert!(matches!(mmu.low_table[0].ty(), DescriptorType::Invalid));

        let block_size = 1 << 25;
        let page_size = 1 << 14;

        let from = VirtualAddress::new(0x12344000000 as *const u8).unwrap();
        let to = PhysicalAddress::new(0x12344000000 as *const u8).unwrap();
        let size = block_size + page_size * 4;
        mmu.map_region(from, to, size, Attributes::Normal, Permissions::RWX)
            .expect("Adding region was successful");

        let low_table = &mut mmu.low_table;
        assert!(matches!(low_table[0].ty(), DescriptorType::Table));
        assert!(matches!(low_table[1].ty(), DescriptorType::Invalid));

        let level1 = low_table[0].get_table().expect("Is a table");
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x12 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x12].get_table().expect("Is a table");

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
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU.initialized = true };

        assert!(matches!(mmu.low_table[0].ty(), DescriptorType::Invalid));

        let block_size = 1 << 25;
        let page_size = 1 << 14;
        let va = 0x12344000000 + page_size * 2044;

        let from = VirtualAddress::new(va as *const u8).unwrap();
        let to = PhysicalAddress::new(0x12344000000 as *const u8).unwrap();
        let size = block_size + page_size * 4;
        mmu.map_region(from, to, size, Attributes::Normal, Permissions::RWX)
            .expect("Adding region was successful");

        let low_table = &mut mmu.low_table;
        assert!(matches!(low_table[0].ty(), DescriptorType::Table));
        assert!(matches!(low_table[1].ty(), DescriptorType::Invalid));

        let level1 = low_table[0].get_table().expect("Is a table");
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x12 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x12].get_table().expect("Is a table");

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
}
