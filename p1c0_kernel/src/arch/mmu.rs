mod early_alloc;

use crate::{
    memory::{
        address::{Address, LogicalAddress, PhysicalAddress, VirtualAddress},
        Attributes, GlobalPermissions, Permissions,
    },
    prelude::*,
};
use early_alloc::{AllocRef, EarlyAllocator};

use core::ops::{Deref, DerefMut};

use aarch64_cpu::{
    asm::barrier,
    registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1, TTBR0_EL1, TTBR1_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

pub const VA_MASK: u64 = (1 << 48) - (1 << 14);
pub const PA_MASK: u64 = (1 << 48) - (1 << 14);
pub const PAGE_BITS: usize = 14;
pub const PAGE_SIZE: usize = 1 << PAGE_BITS;

const PXN: u64 = 1 << 53;
const UXN: u64 = 1 << 54;

const EARLY_ALLOCATOR_SIZE: usize = 128 * 1024;
static EARLY_ALLOCATOR: EarlyAllocator<EARLY_ALLOCATOR_SIZE> = EarlyAllocator::new();

static mut MMU_INITIALIZED: bool = false;

#[derive(Debug, Clone)]
pub enum Error {
    OverlapsExistingMapping(VirtualAddress, TranslationLevel),
    UnalignedAddress,
    InvalidPermissions,
}

const MAIR_ATTR_OFFSET: usize = 2;

fn mair_index_from_attrs(attrs: Attributes) -> u64 {
    ((attrs as u64) & 0x7) << MAIR_ATTR_OFFSET
}

fn attributes_from_mapping(mapping: u64) -> Result<Attributes, Error> {
    Ok(Attributes::try_from((mapping >> MAIR_ATTR_OFFSET) & 0x7).unwrap())
}

fn permission_ap_bits(permissions: GlobalPermissions) -> Result<u64, Error> {
    let GlobalPermissions {
        privileged,
        unprivileged,
    } = permissions;
    Ok(match (privileged, unprivileged) {
        (Permissions::RW | Permissions::RWX, Permissions::None) => 0b00 << 6,
        (Permissions::RX | Permissions::RO, Permissions::None) => 0b10 << 6,
        (Permissions::RW, Permissions::RW | Permissions::RWX) => 0b01 << 6,
        (Permissions::RX | Permissions::RO, Permissions::RX | Permissions::RO) => 0b11 << 6,
        _ => {
            return Err(Error::InvalidPermissions);
        }
    })
}

fn permission_nx_bits(permissions: GlobalPermissions) -> Result<u64, Error> {
    let GlobalPermissions {
        privileged,
        unprivileged,
    } = permissions;

    let pxn = match privileged {
        Permissions::RWX | Permissions::RX => 0,
        _ => PXN,
    };

    let uxn = match unprivileged {
        Permissions::RWX | Permissions::RX => 0,
        _ => UXN,
    };

    Ok(pxn | uxn)
}

fn permission_bits(permissions: GlobalPermissions) -> Result<u64, Error> {
    Ok(permission_ap_bits(permissions)? | permission_nx_bits(permissions)?)
}

fn permissions_from_mapping(mapping: u64) -> GlobalPermissions {
    let pxn = (mapping & PXN) != 0;
    let uxn = (mapping & UXN) != 0;
    let writable = (mapping & (0b10 << 6)) == 0;
    let user = (mapping & (0b01 << 6)) != 0;

    match (pxn, uxn, writable, user) {
        (false, _, false, false) => GlobalPermissions {
            privileged: Permissions::RX,
            unprivileged: Permissions::None,
        },
        (true, _, false, false) => GlobalPermissions {
            privileged: Permissions::RO,
            unprivileged: Permissions::None,
        },
        (false, _, true, false) => GlobalPermissions {
            privileged: Permissions::RWX,
            unprivileged: Permissions::None,
        },
        (true, _, true, false) => GlobalPermissions {
            privileged: Permissions::RW,
            unprivileged: Permissions::None,
        },
        (false, false, false, true) => GlobalPermissions {
            privileged: Permissions::RX,
            unprivileged: Permissions::RX,
        },
        (true, false, false, true) => GlobalPermissions {
            privileged: Permissions::RO,
            unprivileged: Permissions::RX,
        },
        (false, true, false, true) => GlobalPermissions {
            privileged: Permissions::RX,
            unprivileged: Permissions::RO,
        },
        (true, true, false, true) => GlobalPermissions {
            privileged: Permissions::RO,
            unprivileged: Permissions::RO,
        },
        (_, false, true, true) => GlobalPermissions {
            privileged: Permissions::RW,
            unprivileged: Permissions::RWX,
        },
        (false, true, true, true) => GlobalPermissions {
            privileged: Permissions::RWX,
            unprivileged: Permissions::RW,
        },
        (true, true, true, true) => GlobalPermissions {
            privileged: Permissions::RW,
            unprivileged: Permissions::RW,
        },
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
pub struct DescriptorEntry(u64);

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
        let early = !is_initialized();
        let table_addr = if early {
            Box::leak(Box::new_in(
                LevelTable::new(),
                AllocRef::new(&EARLY_ALLOCATOR),
            ))
        } else {
            // This gives a logical memory address, we need to translate it to its physical
            // address for the table
            let table = Box::new(LevelTable::new());
            let kla = LogicalAddress::try_from_ptr(Box::leak(table) as *mut LevelTable as *mut u8)
                .expect("Level table is aligned to 16kB");
            kla.into_physical().as_ptr() as *mut u8 as *mut LevelTable
        };
        let early_bit = if early { Self::EARLY_BIT } else { 0 };
        Self(Self::VALID_BIT | Self::TABLE_BIT | (table_addr as u64 & PA_MASK) | early_bit)
    }

    fn new_block_desc(
        physical_addr: PhysicalAddress,
        attributes: Attributes,
        permissions: GlobalPermissions,
    ) -> Result<Self, Error> {
        Ok(Self(
            Self::VALID_BIT
                | Self::ACCESS_FLAG
                | (physical_addr.as_usize() as u64 & PA_MASK)
                | mair_index_from_attrs(attributes)
                | Self::SHAREABILITY
                | permission_bits(permissions)?,
        ))
    }

    fn new_page_desc(
        physical_addr: PhysicalAddress,
        attributes: Attributes,
        permissions: GlobalPermissions,
    ) -> Result<Self, Error> {
        Ok(Self(
            Self::VALID_BIT
                | Self::TABLE_BIT
                | Self::PAGE_BIT
                | Self::ACCESS_FLAG
                | (physical_addr.as_u64() & PA_MASK)
                | mair_index_from_attrs(attributes)
                | Self::SHAREABILITY
                | permission_bits(permissions)?,
        ))
    }

    fn get_table(&mut self) -> Option<&mut LevelTable> {
        match self.ty() {
            DescriptorType::Table => {
                let table_ptr = (self.0 & VA_MASK) as *mut LevelTable;
                if is_initialized() {
                    // If the MMU is initialized we have a physical pointer that should have a
                    // corresponding logical address.
                    let pa = PhysicalAddress::try_from_ptr(table_ptr as *const _)
                        .expect("Tables should always be aligned to 16kB");
                    let table_ptr = pa
                        .try_into_logical()
                        .map(|kla| kla.as_ptr() as *mut LevelTable)
                        .expect("table ptr is not a logical address");
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
                Some(unsafe { PhysicalAddress::new_unchecked((self.0 & PA_MASK) as *const u8) })
            }
            _ => None,
        }
    }

    fn attrs(&self) -> Option<Attributes> {
        match self.ty() {
            DescriptorType::Page | DescriptorType::Block => attributes_from_mapping(self.0).ok(),
            _ => None,
        }
    }

    fn permissions(&self) -> Option<GlobalPermissions> {
        match self.ty() {
            DescriptorType::Page | DescriptorType::Block => Some(permissions_from_mapping(self.0)),
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
        matches!(*self, TranslationLevel::Level2)
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
        let va = va.as_ptr() as usize;
        (va & self.va_mask()) >> self.offset()
    }

    fn is_address_aligned(&self, va: VirtualAddress) -> bool {
        let va = va.as_ptr() as usize;
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
/// Happy coincidence this is just enough memory that fits in a single 16KB page
#[repr(C, align(0x4000))]
pub struct LevelTable {
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
    pub const fn new() -> Self {
        Self {
            table: [INVALID_DESCRIPTOR; 2048],
        }
    }
    pub fn unmap_region(&mut self, va: VirtualAddress, size: usize) -> Result<(), Error> {
        log_debug!("Removing mapping at {:?}, size 0x{:x}", va, size);
        self.unmap_region_internal(va, size, TranslationLevel::Level0)
    }

    pub fn unmap_region_internal(
        &mut self,
        mut va: VirtualAddress,
        mut size: usize,
        level: TranslationLevel,
    ) -> Result<(), Error> {
        // Size needs to be aligned to page size
        if (size % PAGE_SIZE) != 0 {
            size = size + PAGE_SIZE - (size % PAGE_SIZE);
        }

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

            let entry_type = descriptor_entry.ty();

            if !matches!(entry_type, DescriptorType::Invalid) {
                if (aligned && (chunk_size == entry_size))
                    || matches!(entry_type, DescriptorType::Page)
                {
                    *descriptor_entry = DescriptorEntry::new_invalid();
                } else {
                    if matches!(entry_type, DescriptorType::Block) {
                        // Turn it into a table, then go in and remove whatever is left
                        let attrs = descriptor_entry.attrs().unwrap();
                        let permissions = descriptor_entry.permissions().unwrap();
                        let pa = descriptor_entry.pa().unwrap();

                        // Now it is a table!
                        *descriptor_entry = DescriptorEntry::new_table_desc();

                        descriptor_entry
                            .get_table()
                            .expect("Is a table")
                            .map_region_internal(
                                va,
                                pa,
                                entry_size,
                                attrs,
                                permissions,
                                level.next(),
                            )?;
                    }

                    // Now unmap what's left
                    descriptor_entry
                        .get_table()
                        .expect("Is a table")
                        .unmap_region_internal(va, chunk_size, level.next())?;
                }
            }

            unsafe {
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
        size: usize,
        attributes: Attributes,
        permissions: GlobalPermissions,
    ) -> Result<(), Error> {
        log_debug!(
            "Adding mapping from {:?} to {:?}, size 0x{:x}",
            va,
            pa,
            size
        );

        self.map_region_internal(
            va,
            pa,
            size,
            attributes,
            permissions,
            TranslationLevel::Level0,
        )
    }

    fn map_region_internal(
        &mut self,
        mut va: VirtualAddress,
        mut pa: PhysicalAddress,
        mut size: usize,
        attributes: Attributes,
        permissions: GlobalPermissions,
        level: TranslationLevel,
    ) -> Result<(), Error> {
        // Size needs to be aligned to page size
        if (size % PAGE_SIZE) != 0 {
            size = size + PAGE_SIZE - (size % PAGE_SIZE);
        }

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
                // FIXME(javier-varez): Check permission bits and attributes too!
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
                    DescriptorEntry::new_page_desc(pa, attributes, permissions)?
                } else {
                    DescriptorEntry::new_block_desc(pa, attributes, permissions)?
                };
            } else {
                if matches!(descriptor_entry.ty(), DescriptorType::Invalid) {
                    *descriptor_entry = DescriptorEntry::new_table_desc();
                }

                descriptor_entry
                    .get_table()
                    .expect("Is a table")
                    .map_region_internal(
                        va,
                        pa,
                        chunk_size,
                        attributes,
                        permissions,
                        level.next(),
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
}

pub fn switch_process_translation_table(low_table: &LevelTable) {
    let va = LogicalAddress::try_from_ptr(low_table.as_ptr() as *mut _)
        .expect("Level table not aligned!!");
    let pa = va.into_physical();

    TTBR0_EL1.set_baddr(pa.as_u64());

    barrier::dsb(barrier::ISHST);
    barrier::isb(barrier::SY);

    // TODO(javier-varez): We need better granularity here for performance
    flush_tlb()
}

pub fn flush_tlb() {
    #[cfg(all(not(test), target_arch = "aarch64"))]
    unsafe {
        core::arch::asm!("dsb ishst\n", "tlbi vmalle1\n", "dsb ish\n", "isb\n");
    }
}

pub fn flush_tlb_page(addr: VirtualAddress) {
    assert!(addr.is_page_aligned());

    #[cfg(all(not(test), target_arch = "aarch64"))]
    unsafe {
        core::arch::asm!("dsb ishst\n", "tlbi vaae1, x0\n", "dsb ish\n", "isb\n", in("x0") addr.page_number());
    }
}

pub fn initialize(high_table: &LevelTable, low_table: &LevelTable) {
    if unsafe { MMU_INITIALIZED } {
        panic!("MMU Already initialized!");
    }

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

    TTBR0_EL1.set_baddr(low_table.table.as_ptr() as u64);
    TTBR1_EL1.set_baddr(high_table.table.as_ptr() as u64);

    barrier::dsb(barrier::ISHST);
    barrier::isb(barrier::SY);

    // Actually enable the MMU
    SCTLR_EL1.modify(SCTLR_EL1::M::Enable + SCTLR_EL1::C::Cacheable + SCTLR_EL1::I::Cacheable);

    flush_tlb();

    if matches!(
        SCTLR_EL1.read_as_enum(SCTLR_EL1::M),
        Some(SCTLR_EL1::M::Value::Enable)
    ) {
        log_info!("MMU enabled");
    } else {
        log_error!("Error enabling MMU");
    }

    // It is safe to set it here, because we are in a single-threaded context
    unsafe { MMU_INITIALIZED = true };
}

pub fn is_initialized() -> bool {
    unsafe { MMU_INITIALIZED }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn single_page_mapping() {
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU_INITIALIZED = true };

        let mut table = LevelTable::new();

        let from = VirtualAddress::try_from_ptr(0x012345678000 as *const u8).unwrap();
        let to = PhysicalAddress::try_from_ptr(0x012345678000 as *const u8).unwrap();
        let size = 1 << 14;
        table
            .map_region(
                from,
                to,
                size,
                Attributes::Normal,
                GlobalPermissions::new_only_privileged(Permissions::RWX),
            )
            .expect("Adding region was successful");

        assert!(matches!(table[0].ty(), DescriptorType::Table));
        assert!(matches!(table[1].ty(), DescriptorType::Invalid));

        let level1 = table[0].get_table().expect("Is a table");
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
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU_INITIALIZED = true };

        let mut table = LevelTable::new();

        let from = VirtualAddress::try_from_ptr(0x12344000000 as *const u8).unwrap();
        let to = PhysicalAddress::try_from_ptr(0x12344000000 as *const u8).unwrap();
        let size = 1 << 25;
        table
            .map_region(
                from,
                to,
                size,
                Attributes::Normal,
                GlobalPermissions::new_only_privileged(Permissions::RWX),
            )
            .expect("Adding region was successful");

        assert!(matches!(table[0].ty(), DescriptorType::Table));
        assert!(matches!(table[1].ty(), DescriptorType::Invalid));

        let level1 = table[0].get_table().expect("Is a table");
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
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU_INITIALIZED = true };

        let mut table = LevelTable::new();

        let block_size = 1 << 25;
        let page_size = 1 << 14;

        let from = VirtualAddress::try_from_ptr(0x12344000000 as *const u8).unwrap();
        let to = PhysicalAddress::try_from_ptr(0x12344000000 as *const u8).unwrap();
        let size = block_size + page_size * 4;
        table
            .map_region(
                from,
                to,
                size,
                Attributes::Normal,
                GlobalPermissions::new_only_privileged(Permissions::RWX),
            )
            .expect("Adding region was successful");

        assert!(matches!(table[0].ty(), DescriptorType::Table));
        assert!(matches!(table[1].ty(), DescriptorType::Invalid));

        let level1 = table[0].get_table().expect("Is a table");
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
                let to_usize = to.as_usize() + block_size + page_size * idx;
                let to = PhysicalAddress::try_from_ptr(to_usize as *const _)
                    .expect("Address is aligned");
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }
    }

    #[test]
    fn large_unaligned_block_mapping() {
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU_INITIALIZED = true };

        let mut table = LevelTable::new();

        let block_size = 1 << 25;
        let page_size = 1 << 14;
        let va = 0x12344000000 + page_size * 2044;

        let from = VirtualAddress::try_from_ptr(va as *const u8).unwrap();
        let to = PhysicalAddress::try_from_ptr(0x12344000000 as *const u8).unwrap();
        let size = block_size + page_size * 4;
        table
            .map_region(
                from,
                to,
                size,
                Attributes::Normal,
                GlobalPermissions::new_only_privileged(Permissions::RWX),
            )
            .expect("Adding region was successful");

        assert!(matches!(table[0].ty(), DescriptorType::Table));
        assert!(matches!(table[1].ty(), DescriptorType::Invalid));

        let level1 = table[0].get_table().expect("Is a table");
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
                let to_usize = to.as_usize() + page_size * 4;
                let to = PhysicalAddress::try_from_ptr(to_usize as *const _)
                    .expect("Address is aligned");
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level3 = level2[0x1a2].get_table().expect("Is a table");

        for (idx, desc) in level3.table.iter().enumerate() {
            if idx >= 2044 {
                assert!(matches!(desc.ty(), DescriptorType::Page));
                let to_usize = to.as_usize() + page_size * (idx - 2044);
                let to = PhysicalAddress::try_from_ptr(to_usize as *const _)
                    .expect("Address is aligned");
                assert_eq!(desc.pa(), Some(to));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }
    }

    #[test]
    fn unmap_single_page() {
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU_INITIALIZED = true };

        let mut table = LevelTable::new();

        let from = VirtualAddress::try_from_ptr(0x012345678000 as *const u8).unwrap();
        let to = PhysicalAddress::try_from_ptr(0x012345678000 as *const u8).unwrap();
        let size = 1 << 14;
        table
            .map_region(
                from,
                to,
                size,
                Attributes::Normal,
                GlobalPermissions::new_only_privileged(Permissions::RWX),
            )
            .expect("Could add region");

        table.unmap_region(from, size).expect("Could remove region");

        let table = &mut table;
        assert!(matches!(table[0].ty(), DescriptorType::Table));
        assert!(matches!(table[1].ty(), DescriptorType::Invalid));

        let level1 = table[0].get_table().expect("Is a table");
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
                // This has been removed
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }
    }

    #[test]
    fn unmap_multiple_blocks() {
        // Let's trick the test to use the global allocator instead of the early allocator. On
        // tests our assumptions don't hold for the global allocator, so we need to make sure to
        // use an adequate allocator.
        unsafe { MMU_INITIALIZED = true };

        let mut table = LevelTable::new();

        let from = VirtualAddress::try_from_ptr(0x10000000000 as *const u8).unwrap();
        let to = PhysicalAddress::try_from_ptr(0x10000000000 as *const u8).unwrap();
        let size = 0x800000000;
        table
            .map_region(
                from,
                to,
                size,
                Attributes::Normal,
                GlobalPermissions::new_only_privileged(Permissions::RWX),
            )
            .expect("Could add region");

        table.unmap_region(from, size).expect("Could remove region");

        let table = &mut table;
        assert!(matches!(table[0].ty(), DescriptorType::Table));
        assert!(matches!(table[1].ty(), DescriptorType::Invalid));

        let level1 = table[0].get_table().expect("Is a table");
        for (idx, desc) in level1.table.iter().enumerate() {
            if idx == 0x10 {
                assert!(matches!(desc.ty(), DescriptorType::Table));
            } else {
                assert!(matches!(desc.ty(), DescriptorType::Invalid));
            }
        }

        let level2 = level1[0x10].get_table().expect("Is a table");

        for desc in level2.table.iter() {
            assert!(matches!(desc.ty(), DescriptorType::Invalid));
        }
    }
}
