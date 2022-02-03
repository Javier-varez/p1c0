use super::address::{LogicalAddress, VirtualAddress};

/// This is the base address for logical addresses.
pub const KERNEL_LOGICAL_BASE: LogicalAddress =
    unsafe { LogicalAddress::new_unchecked(0xFFFF020000000000 as *const u8) };
pub const KERNEL_LOGICAL_SIZE: usize = 1024 * 1024 * 1024 * 1024; // 1 TB

pub const ADT_VIRTUAL_BASE: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xFFFF000000000000 as *const u8) };
