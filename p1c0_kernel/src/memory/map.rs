use super::{
    address::{LogicalAddress, PhysicalAddress, VirtualAddress},
    Permissions,
};
use crate::arch::mmu::PAGE_SIZE;
use crate::memory::GlobalPermissions;

/// This is the base address for logical addresses.
pub const KERNEL_LOGICAL_BASE: LogicalAddress =
    unsafe { LogicalAddress::new_unchecked(0xFFFF020000000000 as *const u8) };
pub const KERNEL_LOGICAL_SIZE: usize = 128 * 1024 * 1024 * 1024 * 1024; // 128 TB

pub const ADT_VIRTUAL_BASE: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xFFFF000000000000 as *const u8) };

/// Last 4GB are reserved for MMIO
pub const MMIO_BASE: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xFFFFFFFF00000000 as *const u8) };
pub const MMIO_SIZE: usize = 4 * 1024 * 1024 * 1024 - PAGE_SIZE;

/// Last page is used for fast mapping into the kernel address space.
pub const FASTMAP_PAGE: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xFFFFFFFFFFFFC000 as *const u8) };

extern "C" {
    static _text_start: u8;
    static _text_end: u8;
    static _rodata_start: u8;
    static _rodata_end: u8;
    static _data_start: u8;
    static _data_end: u8;
    static _arena_start: u8;
    static _arena_end: u8;
    static _payload_start: u8;
    static _payload_end: u8;
}

#[derive(Debug, Clone, Copy)]
pub enum KernelSectionId {
    Text,
    RoData,
    Data,
    Arena,
    Payload,
}

pub const ALL_SECTIONS: [KernelSectionId; 5] = [
    KernelSectionId::Text,
    KernelSectionId::RoData,
    KernelSectionId::Data,
    KernelSectionId::Arena,
    KernelSectionId::Payload,
];

pub struct KernelSection {
    name: &'static str,
    start: PhysicalAddress,
    size_bytes: usize,
    permissions: GlobalPermissions,
}

impl KernelSection {
    pub fn from_id(id: KernelSectionId) -> Self {
        let (name, start, end, permissions) = unsafe {
            match id {
                KernelSectionId::Text => (
                    ".text",
                    &_text_start as *const u8,
                    &_text_end as *const u8,
                    GlobalPermissions::new_only_privileged(Permissions::RX),
                ),
                KernelSectionId::RoData => (
                    ".rodata",
                    &_rodata_start as *const _,
                    &_rodata_end as *const _,
                    GlobalPermissions::new_only_privileged(Permissions::RO),
                ),
                KernelSectionId::Data => (
                    ".data",
                    &_data_start as *const _,
                    &_data_end as *const _,
                    GlobalPermissions::new_only_privileged(Permissions::RW),
                ),
                KernelSectionId::Arena => (
                    ".arena",
                    &_arena_start as *const _,
                    &_arena_end as *const _,
                    GlobalPermissions::new_only_privileged(Permissions::RW),
                ),
                KernelSectionId::Payload => (
                    ".payload",
                    &_payload_start as *const _,
                    &_payload_end as *const _,
                    GlobalPermissions::new_only_privileged(Permissions::RO),
                ),
            }
        };

        let size_bytes = unsafe { end.offset_from(start) as usize };
        let start = if crate::arch::mmu::is_initialized() {
            // After relocation this is a logical address.
            LogicalAddress::try_from_ptr(start)
                .expect("KernelSection should be logical after reloc")
                .into_physical()
        } else {
            PhysicalAddress::try_from_ptr(start).expect("KernelSection should be aligned")
        };

        Self {
            name,
            start,
            size_bytes,
            permissions,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn pa(&self) -> PhysicalAddress {
        self.start
    }

    pub fn la(&self) -> LogicalAddress {
        self.start
            .try_into_logical()
            .expect("KernelSection should be convertible to logical address")
    }

    pub fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    pub fn permissions(&self) -> GlobalPermissions {
        self.permissions
    }
}
