pub mod address;
pub mod address_space;
pub mod kalloc;
pub mod map;
pub mod physical_page_allocator;

use crate::{
    arch::{
        self,
        mmu::{PAGE_BITS, PAGE_SIZE},
    },
    sync::spinlock::{SpinLock, SpinLockGuard},
};
use address::{Address, LogicalAddress, PhysicalAddress, VirtualAddress};
use address_space::MemoryRange;
use physical_page_allocator::{PhysicalMemoryRegion, PhysicalPageAllocator};

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
    TranslationError,
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum AllocPolicy {
    ZeroFill,
    None,
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
    None,
    RWX,
    RW,
    RX,
    RO,
}

#[derive(Clone, Copy, Debug)]
pub struct GlobalPermissions {
    pub unprivileged: Permissions,
    pub privileged: Permissions,
}

impl GlobalPermissions {
    pub fn new_only_privileged(privileged: Permissions) -> Self {
        Self {
            unprivileged: Permissions::None,
            privileged,
        }
    }

    pub fn new_for_process(unprivileged: Permissions) -> Self {
        Self {
            unprivileged,
            privileged: match unprivileged {
                Permissions::RWX => Permissions::RW,
                Permissions::RX => Permissions::RO,
                perm => perm,
            },
        }
    }
}

static MEMORY_MANAGER: SpinLock<MemoryManager> = SpinLock::new(MemoryManager::new());

pub struct MemoryManager {
    kernel_address_space: address_space::KernelAddressSpace,
    physical_page_allocator: PhysicalPageAllocator,
}

impl MemoryManager {
    const fn new() -> Self {
        Self {
            kernel_address_space: address_space::KernelAddressSpace::new(),
            physical_page_allocator: PhysicalPageAllocator::new(),
        }
    }

    fn add_kernel_mapping(&mut self, section: &map::KernelSection) -> Result<(), Error> {
        let high_table = self.kernel_address_space.high_table();
        let pa = section.pa();
        let va = section.la().into_virtual();
        high_table.map_region(
            va,
            pa,
            section.size_bytes(),
            Attributes::Normal,
            section.permissions(),
        )?;
        Ok(())
    }

    fn add_kernel_mappings(&mut self) -> Result<(), Error> {
        for section_id in map::ALL_SECTIONS.iter() {
            let section = map::KernelSection::from_id(*section_id);
            self.add_kernel_mapping(&section)?;
        }
        Ok(())
    }

    fn add_default_mappings(&mut self) {
        let adt = crate::adt::get_adt().unwrap();
        let chosen = adt.find_node("/chosen").expect("There is a chosen node");
        let dram_base = chosen
            .find_property("dram-base")
            .and_then(|prop| prop.usize_value().ok())
            .map(|addr| addr as *const u8)
            .expect("There is no dram base");
        let dram_size = chosen
            .find_property("dram-size")
            .and_then(|prop| prop.usize_value().ok())
            .expect("There is no dram base");

        // Add initial identity mapping. To be removed after relocation.
        self.kernel_address_space
            .low_table()
            .map_region(
                VirtualAddress::try_from_ptr(dram_base)
                    .expect("Address is not aligned to page size"),
                PhysicalAddress::try_from_ptr(dram_base)
                    .expect("Address is not aligned to page size"),
                dram_size,
                Attributes::Normal,
                GlobalPermissions::new_only_privileged(Permissions::RWX),
            )
            .expect("Mappings overlap");

        self.add_kernel_mappings()
            .expect("Kernel can not be mapped");

        // Map mmio ranges as defined in the ADT
        let root_address_cells = adt.find_node("/").and_then(|node| node.get_address_cells());
        let node = adt.find_node("/arm-io").expect("There is not an arm-io");
        let range_iter = node.range_iter(root_address_cells);
        for range in range_iter {
            let mmio_region_base = range.get_parent_addr() as *const u8;
            let mmio_region_size = range.get_size();
            self.kernel_address_space
                .low_table()
                .map_region(
                    VirtualAddress::try_from_ptr(mmio_region_base)
                        .expect("Address is not aligned to page size"),
                    PhysicalAddress::try_from_ptr(mmio_region_base)
                        .expect("Address is not aligned to page size"),
                    mmio_region_size,
                    Attributes::DevicenGnRnE,
                    GlobalPermissions::new_only_privileged(Permissions::RWX),
                )
                .expect("Mappings overlap");
        }
    }

    pub fn remove_identity_mappings(&mut self) {
        let low_table = self.kernel_address_space.low_table();

        let adt = crate::adt::get_adt().unwrap();
        let chosen = adt.find_node("/chosen").expect("There is a chosen node");
        let dram_base = chosen
            .find_property("dram-base")
            .and_then(|prop| prop.usize_value().ok())
            .map(|addr| addr as *const u8)
            .expect("There is no dram base");
        let dram_size = chosen
            .find_property("dram-size")
            .and_then(|prop| prop.usize_value().ok())
            .expect("There is no dram base");

        low_table
            .unmap_region(
                VirtualAddress::try_from_ptr(dram_base)
                    .expect("Address is not aligned to page size"),
                dram_size,
            )
            .expect("Cannot unmap DRAM identity-map");

        // Map mmio ranges as defined in the ADT
        let root_address_cells = adt.find_node("/").and_then(|node| node.get_address_cells());
        let node = adt.find_node("/arm-io").expect("There is not an arm-io");
        let range_iter = node.range_iter(root_address_cells);
        for range in range_iter {
            let mmio_region_base = range.get_parent_addr() as *const u8;
            let mmio_region_size = range.get_size();
            low_table
                .unmap_region(
                    VirtualAddress::try_from_ptr(mmio_region_base)
                        .expect("Address is not aligned to page size"),
                    mmio_region_size,
                )
                .expect("Cannot unmap MMIO identity-map");
        }
    }

    /// # Safety
    ///   Should only be called once on system boot before the MMU is initialized (done by this
    ///   function)
    pub unsafe fn early_init() {
        // At this point we cannot use the lock because the memory manager is not initialized
        MEMORY_MANAGER.access_inner_without_locking(|mem_mgr| {
            // Create the default mappings before early initialization
            mem_mgr.add_default_mappings();

            // Initialize the MMU with the tables
            let (high_table, low_table) = mem_mgr.kernel_address_space.tables();
            arch::mmu::initialize(high_table, low_table);
        });
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
        self.kernel_address_space
            .high_table()
            .map_region(
                map::ADT_VIRTUAL_BASE,
                device_tree,
                device_tree_size,
                Attributes::Normal,
                GlobalPermissions::new_only_privileged(Permissions::RO),
            )
            .expect("Boot args can be mapped");

        // Now unmap identity mapping
        self.remove_identity_mappings();

        let adt = crate::adt::get_adt().unwrap();
        let chosen = adt.find_node("/chosen").expect("There is a chosen node");
        let dram_base = chosen
            .find_property("dram-base")
            .and_then(|prop| prop.usize_value().ok())
            .and_then(|addr| PhysicalAddress::try_from_ptr(addr as *const u8).ok())
            .expect("There is a dram base");
        let dram_size = chosen
            .find_property("dram-size")
            .and_then(|prop| prop.usize_value().ok())
            .expect("There is a dram base");

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

        let la = logical_range.la;
        let size = logical_range.size_bytes;
        let attributes = logical_range.attributes;
        let permissions = GlobalPermissions::new_only_privileged(logical_range.permissions);

        self.kernel_address_space
            .high_table()
            .map_region(
                la.into_virtual(),
                la.into_physical(),
                size,
                attributes,
                permissions,
            )
            .expect("MMU cannot map requested region");

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
            .allocate_io_range(name, pa, size_bytes)?;

        self.kernel_address_space
            .high_table()
            .map_region(
                va,
                pa,
                size_bytes,
                Attributes::DevicenGnRnE,
                GlobalPermissions::new_only_privileged(Permissions::RW),
            )
            .expect("MMU cannot map requested region");

        Ok(va)
    }

    pub fn remove_mapping_by_name(&mut self, name: &str) -> Result<(), Error> {
        let (table, range) = self.kernel_address_space.remove_range_by_name(name)?;
        table.unmap_region(range.virtual_address(), range.size_bytes())?;

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
                section.permissions().privileged,
                None,
            )?;
        }
        Ok(())
    }

    pub fn request_any_pages(
        &mut self,
        num_pages: usize,
        policy: AllocPolicy,
    ) -> Result<PhysicalMemoryRegion, Error> {
        let pmr = self.physical_page_allocator.request_any_pages(num_pages)?;

        if policy == AllocPolicy::ZeroFill {
            for page_idx in 0..pmr.num_pages() {
                let pa = unsafe { pmr.base_address().offset(page_idx * PAGE_SIZE) };
                self.do_with_fast_map(
                    pa,
                    GlobalPermissions::new_only_privileged(Permissions::RW),
                    |va| unsafe { core::ptr::write_bytes(va.as_mut_ptr(), 0u8, PAGE_SIZE) },
                );
            }
        }

        Ok(pmr)
    }

    pub fn release_pages(
        &mut self,
        physical_memory_region: PhysicalMemoryRegion,
    ) -> Result<(), Error> {
        self.physical_page_allocator
            .release_pages(physical_memory_region)?;
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

    pub fn do_with_fast_map<T>(
        &mut self,
        pa: PhysicalAddress,
        permissions: GlobalPermissions,
        mut f: impl FnMut(VirtualAddress) -> T,
    ) -> T {
        self.kernel_address_space
            .fast_page_map(pa, permissions, Attributes::Normal)
            .unwrap();

        let val = f(map::FASTMAP_PAGE);

        self.kernel_address_space.fast_page_unmap().unwrap();
        val
    }

    pub fn map_kernel_low_pages(&mut self) {
        arch::mmu::switch_process_translation_table(self.kernel_address_space.low_table());
    }

    pub fn translate_kernel_address(&self, va: VirtualAddress) -> Result<PhysicalAddress, Error> {
        if !va.is_high_address() {
            return Err(Error::TranslationError);
        }

        Ok(self.kernel_address_space.resolve_address(va)?)
    }
}
