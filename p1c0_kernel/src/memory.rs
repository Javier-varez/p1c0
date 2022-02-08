pub mod address;
pub mod address_space;
pub mod kalloc;
pub mod map;
pub mod physical_page_allocator;

extern crate alloc;

use crate::arch::{
    self,
    mmu::{PAGE_BITS, PAGE_SIZE},
};

use address::{LogicalAddress, PhysicalAddress, VirtualAddress};
use address_space::KernelAddressSpace;
use map::ADT_VIRTUAL_BASE;
use physical_page_allocator::PhysicalPageAllocator;

use crate::sync::spinlock::{SpinLock, SpinLockGuard};

use self::address_space::MemoryRange;

pub fn num_pages_from_bytes(bytes: usize) -> usize {
    if bytes & (PAGE_SIZE - 1) == 0 {
        bytes >> PAGE_BITS
    } else {
        (bytes >> PAGE_BITS) + 1
    }
}

#[derive(Clone, Debug)]
pub enum Error {
    ArchitectureSpecific(arch::mmu::Error),
    AddressSpaceError(address_space::Error),
    PageAllocationError(physical_page_allocator::Error),
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

impl From<physical_page_allocator::Error> for Error {
    fn from(inner: physical_page_allocator::Error) -> Self {
        Error::PageAllocationError(inner)
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

static MEMORY_MANAGER: SpinLock<MemoryManager> = SpinLock::new(MemoryManager::new());

pub struct MemoryManager {
    kernel_address_space: KernelAddressSpace,
    physical_page_allocator: PhysicalPageAllocator,
}

impl MemoryManager {
    const fn new() -> Self {
        Self {
            kernel_address_space: KernelAddressSpace::new(),
            physical_page_allocator: PhysicalPageAllocator::new(),
        }
    }

    /// # Safety
    ///   Should only be called once on system boot before the MMU is initialized (done by this
    ///   function)
    pub unsafe fn early_init() {
        arch::mmu::initialize();
    }

    pub fn instance() -> SpinLockGuard<'static, Self> {
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
        let device_tree =
            PhysicalAddress::from_unaligned_ptr(device_tree as *const _).align_to_page();
        arch::mmu::MMU
            .map_region(
                ADT_VIRTUAL_BASE,
                device_tree,
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

        arch::mmu::MMU
            .unmap_region(VirtualAddress::try_from_ptr(dram_base).unwrap(), dram_size)
            .expect("Can remove identity mapping");

        let dram_base =
            PhysicalAddress::try_from_ptr(dram_base).expect("The DRAM base is not page aligned");
        self.initialize_physical_page_allocator(
            dram_base,
            dram_size,
            device_tree,
            device_tree_size,
        )
        .expect("Could not initialize physical_page_allocator");
    }

    pub fn map_logical(
        &mut self,
        name: &str,
        la: LogicalAddress,
        size_bytes: usize,
        attributes: Attributes,
        permissions: Permissions,
    ) -> Result<(), Error> {
        // Request pages from the PhysicalPageAllocator
        let region = self
            .physical_page_allocator
            .request_pages(la.into_physical(), num_pages_from_bytes(size_bytes))?;

        // Getting the logical range must succeed because we got ownership of the pages and this is
        // a logical mapping (one-to-one address)
        let logical_range = self
            .kernel_address_space
            .add_logical_range(name, la, size_bytes, attributes, permissions, Some(region))
            .expect("Error mapping logical range");

        unsafe {
            arch::mmu::MMU
                .map_region(
                    logical_range.la.into_virtual(),
                    logical_range.la.into_physical(),
                    logical_range.size_bytes,
                    logical_range.attributes,
                    logical_range.permissions,
                )
                .expect("MMU cannot map requested region")
        };

        Ok(())
    }

    // Maps memory in the virtual memory region (out of the logical region) as device memory with
    // RW permissions
    pub fn map_io(
        &mut self,
        name: &str,
        pa: PhysicalAddress,
        size_bytes: usize,
    ) -> Result<VirtualAddress, Error> {
        let va = self
            .kernel_address_space
            .allocate_io_range(name, size_bytes)?;

        unsafe {
            arch::mmu::MMU
                .map_region(
                    va,
                    pa,
                    size_bytes,
                    Attributes::DevicenGnRnE,
                    Permissions::RW,
                )
                .expect("MMU cannot map requested region")
        };

        Ok(va)
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
                None,
            )?;
        }
        Ok(())
    }

    fn initialize_physical_page_allocator(
        &mut self,
        dram_base: PhysicalAddress,
        dram_size: usize,
        device_tree_base: PhysicalAddress,
        device_tree_size: usize,
    ) -> Result<(), Error> {
        // We initialize the physical page allocator with memory from the DRAM
        let dram_pages = num_pages_from_bytes(dram_size);
        self.physical_page_allocator
            .add_region(dram_base, dram_pages)?;

        // Remove kernel pages
        for section_id in map::ALL_SECTIONS.iter() {
            let section = map::KernelSection::from_id(*section_id);
            let physical_addr = section.pa();
            let num_pages = num_pages_from_bytes(section.size_bytes());
            self.physical_page_allocator
                .steal_region(physical_addr, num_pages)?;
        }

        // Remove ADT regions
        let device_tree_pages = num_pages_from_bytes(device_tree_size);
        self.physical_page_allocator
            .steal_region(device_tree_base, device_tree_pages)
            .expect("Cannot steal ADT region");

        self.physical_page_allocator.print_regions();

        Ok(())
    }
}
