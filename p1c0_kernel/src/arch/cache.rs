use crate::memory::address::{Address, VirtualAddress};

use aarch64_cpu::asm::barrier::{dmb, SY};

const CACHE_LINE_SIZE: usize = 64;

pub fn invalidate_va_range(mut va: VirtualAddress, size_bytes: usize) {
    let mut num_lines = (size_bytes + CACHE_LINE_SIZE - 1) / CACHE_LINE_SIZE;
    let aligned_va = va.floor_to_alignment(CACHE_LINE_SIZE);
    if va != aligned_va {
        num_lines += 1;
    }

    for i in 0..num_lines {
        unsafe {
            let _va = va.offset(i * CACHE_LINE_SIZE).as_usize();
            #[cfg(target_arch = "aarch64")]
            core::arch::asm!("dc ivac, {}", in(reg) _va);
        }
    }

    // Add barrier operation to ensure the data cache clean completes before the next instructions
    dmb(SY);
}

pub fn clean_va_range(mut va: VirtualAddress, size_bytes: usize) {
    let mut num_lines = (size_bytes + CACHE_LINE_SIZE - 1) / CACHE_LINE_SIZE;
    let aligned_va = va.floor_to_alignment(CACHE_LINE_SIZE);
    if va != aligned_va {
        num_lines += 1;
    }

    for i in 0..num_lines {
        unsafe {
            let _va = va.offset(i * CACHE_LINE_SIZE).as_usize();
            #[cfg(target_arch = "aarch64")]
            core::arch::asm!("dc cvau, {}", in(reg) _va);
        }
    }

    // Add barrier operation to ensure the data cache clean completes before the next instructions
    dmb(SY);
}
