pub mod address;
pub mod kalloc;
pub mod map;

extern crate alloc as alloc;

use crate::arch;
use spin::{Mutex, MutexGuard};

// Reexport these elements
pub use arch::mmu::{Attributes, Permissions};

use address::{PhysicalAddress, VirtualAddress};
use map::ADT_VIRTUAL_BASE;

static MEMORY_MANAGER: Mutex<MemoryManager> = Mutex::new(MemoryManager::new());

pub struct MemoryManager {}

impl MemoryManager {
    const fn new() -> Self {
        Self {}
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
    pub unsafe fn late_init(&self) {
        kalloc::init();

        // Here we relocate the adt
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
}
