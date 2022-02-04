pub mod address;
pub mod address_space;
pub mod kalloc;
pub mod map;

extern crate alloc;

use crate::arch;

use address::{LogicalAddress, PhysicalAddress, VirtualAddress};
use address_space::KernelAddressSpace;
use map::ADT_VIRTUAL_BASE;

use spin::{Mutex, MutexGuard};

use self::address_space::MemoryRange;

#[derive(Clone, Debug)]
pub enum Error {
    ArchitectureSpecific(arch::mmu::Error),
    AddressSpaceError(address_space::Error),
}

impl From<arch::mmu::Error> for Error {
    fn from(inner: arch::mmu::Error) -> Self {
        Error::ArchitectureSpecific(inner)
    }
}

impl From<address_space::Error> for Error {
    fn from(inner: address_space::Error) -> Self {
        Error::AddressSpaceError(inner)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Attributes {
    Normal = 0,
    DevicenGnRnE = 1,
    DevicenGnRE = 2,
}

impl TryFrom<u64> for Attributes {
    type Error = ();
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Attributes::Normal),
            1 => Ok(Attributes::DevicenGnRE),
            2 => Ok(Attributes::DevicenGnRnE),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Permissions {
    RWX,
    RW,
    RX,
    RO,
}

struct PhysicalPage {
    _pfn: usize,
}

static MEMORY_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager::new());

pub struct MemoryManager {
    kernel_address_space: KernelAddressSpace,
}

impl MemoryManager {
    const fn new() -> Self {
        Self {
            kernel_address_space: KernelAddressSpace::new(),
        }
    }

    /// # Safety
    ///   Should only be called once on system boot before the MMU is initialized (done by this
    ///   function)
    pub unsafe fn early_init() {
        arch::mmu::initialize();
    }

    pub fn instance() -> MutexGuard<'static, Self> {
        MEMORY_MANAGER.lock()
    }

    /// # Safety
    ///   Should be called after relocation so that the MemoryManager can remove the identity
    ///   mapping
    pub unsafe fn late_init(&mut self) {
        // Make sure the global allocator is available after this, since we will need it
        kalloc::init();
        self.initialize_address_space()
            .expect("Kernel sections can be mapped");

        // Map ADT
        let boot_args = crate::boot_args::get_boot_args();
        let device_tree =
            boot_args.device_tree as usize - boot_args.virt_base + boot_args.phys_base;
        let device_tree_size = boot_args.device_tree_size as usize;
        arch::mmu::MMU
            .map_region(
                ADT_VIRTUAL_BASE,
                PhysicalAddress::from_unaligned_ptr(device_tree as *const _).align_to_page(),
                device_tree_size,
                Attributes::Normal,
                Permissions::RO,
            )
            .expect("Boot args can be mapped");

        // Now unmap identity mapping
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

        let dram_base = VirtualAddress::try_from_ptr(dram_base).unwrap();
        arch::mmu::MMU
            .unmap_region(dram_base, dram_size)
            .expect("Can remove identity mapping");
    }

    pub fn map_logical(
        &mut self,
        name: &str,
        la: LogicalAddress,
        size_bytes: usize,
        attributes: Attributes,
        permissions: Permissions,
    ) -> Result<(), Error> {
        let range = self.kernel_address_space.add_logical_range(
            name,
            la,
            size_bytes,
            attributes,
            permissions,
        )?;

        unsafe {
            arch::mmu::MMU.map_region(
                range.la.into_virtual(),
                range.la.into_physical(),
                range.size_bytes,
                range.attributes,
                range.permissions,
            )?
        };

        Ok(())
    }

    pub fn remove_mapping_by_name(&mut self, name: &str) -> Result<(), Error> {
        let range = self.kernel_address_space.remove_range_by_name(name)?;
        unsafe {
            arch::mmu::MMU.unmap_region(range.virtual_address(), range.size_bytes())?;
        }

        Ok(())
    }

    fn initialize_address_space(&mut self) -> Result<(), Error> {
        // Add kernel sections that are already mapped
        for section_id in map::ALL_SECTIONS.iter() {
            let section = map::KernelSection::from_id(*section_id);
            self.kernel_address_space.add_logical_range(
                section.name(),
                section.la(),
                section.size_bytes(),
                Attributes::Normal,
                section.permissions(),
            )?;
        }
        Ok(())
    }
}
