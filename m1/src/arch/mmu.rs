extern crate alloc;
use alloc::boxed::Box;
use cortex_a::{
    asm::barrier,
    registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1, TTBR0_EL1, TTBR1_EL1},
};
use tock_registers::interfaces::Writeable;

static mut MMU: MemoryManagementUnit = MemoryManagementUnit::new();

pub enum Attributes {
    Normal = 0,
    DevicenGnRnE = 1,
    DevicenGnRE = 2,
}

pub enum Permissions {
    RWX = 0,
    RW = 1,
    RX = 2,
}

pub struct OutputAddress(*mut ());

#[derive(Eq, PartialEq)]
enum DescriptorType {
    Invalid,
    PageOrBlock,
    Table,
}

#[derive(Clone, Debug)]
#[repr(C)]
struct DescriptorEntry(u64);

const VA_MASK: u64 = (1 << 48) - (1 << 14);
const PA_MASK: u64 = (1 << 48) - (1 << 14);

impl DescriptorEntry {
    const VALID_BIT: u64 = 1 << 0;
    const TABLE_BIT: u64 = 1 << 1;

    const fn new_invalid() -> Self {
        Self(0)
    }

    fn new_table_desc(table_addr: Box<LevelTable>) -> Self {
        let table_addr = Box::leak(table_addr) as *mut _;
        Self(Self::VALID_BIT | Self::TABLE_BIT | (table_addr as u64 & VA_MASK))
    }

    fn new_page_or_block_desc(output_addr: OutputAddress, _attributes: Attributes) -> Self {
        // TODO(javier-varez): Fix attributes
        Self(Self::VALID_BIT | (output_addr.0 as u64 & VA_MASK))
    }

    fn is_table(&self) -> bool {
        self.ty() == DescriptorType::Table
    }

    fn is_block_or_page(&self) -> bool {
        self.ty() == DescriptorType::PageOrBlock
    }

    fn is_invalid(&self) -> bool {
        self.ty() == DescriptorType::Invalid
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
            DescriptorType::PageOrBlock
        } else {
            DescriptorType::Table
        }
    }
}

impl Drop for DescriptorEntry {
    fn drop(&mut self) {
        match self.ty() {
            DescriptorType::Table => {
                // Free table
                let table = self.get_table().expect("Descriptor is a table") as *mut _;
                let table_box = unsafe { Box::from_raw(table) };
                drop(table_box);
            }
            _ => {}
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
enum TranslationLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}

impl TranslationLevel {
    fn is_last(&self) -> bool {
        return *self == TranslationLevel::Level3;
    }

    fn next(&self) -> Self {
        match self {
            &TranslationLevel::Level0 => TranslationLevel::Level1,
            &TranslationLevel::Level1 => TranslationLevel::Level2,
            &TranslationLevel::Level2 => TranslationLevel::Level3,
            &TranslationLevel::Level3 => TranslationLevel::Level3,
        }
    }
}

/// Levels must be aligned at least at 16KB according to the translation granule.
/// In addition, each level must be 11 bits, that's why we have 2048 entries in a level table
#[repr(align(0x4000))]
struct LevelTable {
    table: [DescriptorEntry; 2048],
}

impl LevelTable {
    const fn new() -> Self {
        Self {
            table: [INVALID_DESCRIPTOR; 2048],
        }
    }
}

pub struct MemoryManagementUnit {
    initialized: bool,
    level0: Option<Box<[DescriptorEntry; 2]>>,
}

impl MemoryManagementUnit {
    const fn new() -> Self {
        Self {
            initialized: false,
            level0: None,
        }
    }

    fn init(&mut self) {
        self.level0 = Some(Box::new([INVALID_DESCRIPTOR; 2]));

        let ram_base = 0x10000000000 as *const ();
        let ram_size = 0x800000000;
        self.map_region(
            ram_base,
            ram_base,
            ram_size,
            Attributes::Normal,
            Permissions::RWX,
        );
    }

    pub fn init_and_enable(&mut self) {
        if self.initialized {
            return;
        }
        self.init();
        self.enable();
        self.initialized = true;
    }

    fn enable(&self) {
        MAIR_EL1.write(
            MAIR_EL1::Attr0_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc
                + MAIR_EL1::Attr0_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc
                + MAIR_EL1::Attr1_Device::nonGathering_nonReordering_EarlyWriteAck,
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

        TTBR0_EL1.set_baddr(self.level0.as_ref().unwrap().as_ptr() as u64);
        TTBR1_EL1.set_baddr(self.level0.as_ref().unwrap().as_ptr() as u64);

        unsafe {
            barrier::dsb(barrier::ISHST);
            barrier::isb(barrier::SY);
        }

        SCTLR_EL1.write(SCTLR_EL1::M::Enable + SCTLR_EL1::C::Cacheable + SCTLR_EL1::I::Cacheable);

        unsafe {
            barrier::isb(barrier::SY);
        }
    }

    fn map_region(
        &mut self,
        va: *const (),
        pa: *const (),
        size: usize,
        attributes: Attributes,
        permissions: Permissions,
    ) {
        // TODO(jalv): Implement all levels
        let translation_table = self.level0.as_mut().unwrap();
        let mut va = (va as u64 & VA_MASK) as *const ();
        let mut pa = (pa as u64 & PA_MASK) as *const ();
        let entry_size = 1 << 47;

        let mut remaining_size = size;
        while remaining_size != 0 {
            let index = va as usize >> 47;
            match translation_table[index].ty() {
                DescriptorType::Invalid => {
                    translation_table[index] =
                        DescriptorEntry::new_table_desc(Box::new(LevelTable::new()));
                }
                DescriptorType::Table => {}
                DescriptorType::PageOrBlock => {}
            }

            unsafe {
                pa = pa.add(entry_size);
                va = va.add(entry_size);
            }

            remaining_size = remaining_size.saturating_sub(entry_size);
        }
    }
}

pub fn initialize_mmu() {
    unsafe { MMU.init_and_enable() };
}
