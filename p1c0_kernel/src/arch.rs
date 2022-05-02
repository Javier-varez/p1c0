use crate::log_debug;
use crate::memory::address::VirtualAddress;
use cortex_a::{
    asm,
    registers::{CurrentEL, SPSel},
};
use tock_registers::interfaces::Readable;

pub mod cache;
pub mod exceptions;
pub mod mmu;

#[repr(C)]
pub struct RelaEntry {
    offset: usize,
    ty: usize,
    addend: usize,
}

const R_AARCH64_RELATIVE: usize = 1027;

/// Applies relative offsets during boot to relocate the binary.
///
/// # Safety
///   `rela_start` must point to valid memory, at the start of the relocatable information
///   `rela_len_bytes` must be larger than 0 and indicate the size of the slice in bytes.
///   Other regular conditions must hold when calling thsi function (e.g.: having a valid SP)
#[no_mangle]
pub unsafe extern "C" fn apply_rela(
    base: usize,
    rela_start: *const RelaEntry,
    rela_len_bytes: usize,
) {
    let rela_len = rela_len_bytes / core::mem::size_of::<RelaEntry>();
    let relocations = &*core::ptr::slice_from_raw_parts(rela_start, rela_len);

    for relocation in relocations {
        let ptr = (base + relocation.offset) as *mut usize;
        match relocation.ty {
            R_AARCH64_RELATIVE => *ptr = base + relocation.addend,
            _ => unimplemented!(),
        };
    }
}

/// Applies relative offsets during boot to relocate the binary.
///
/// # Safety
///   `rela_start` must point to valid memory, at the start of the relocatable information
///   `rela_len_bytes` must be larger than 0 and indicate the size of the slice in bytes.
///   Other regular conditions must hold when calling thsi function (e.g.: having a valid SP)
///   old_base must point to an existing mapping to be relocated
pub unsafe fn apply_rela_from_existing(
    old_base: usize,
    base: usize,
    rela_start: *const RelaEntry,
    rela_len_bytes: usize,
) {
    let rela_len = rela_len_bytes / core::mem::size_of::<RelaEntry>();
    let relocations = &*core::ptr::slice_from_raw_parts(rela_start, rela_len);

    for relocation in relocations {
        let ptr = (old_base + relocation.offset) as *mut usize;
        match relocation.ty {
            R_AARCH64_RELATIVE => *ptr = base + relocation.addend,
            _ => unimplemented!(),
        };
    }
}

#[derive(Debug, Clone)]
pub enum ExceptionLevel {
    Application,
    OS,
    Hypervisor,
    SecureMonitor,
}

pub fn get_exception_level() -> ExceptionLevel {
    let el = CurrentEL.read_as_enum(CurrentEL::EL).unwrap();

    match el {
        CurrentEL::EL::Value::EL0 => ExceptionLevel::Application,
        CurrentEL::EL::Value::EL1 => ExceptionLevel::OS,
        CurrentEL::EL::Value::EL2 => ExceptionLevel::Hypervisor,
        CurrentEL::EL::Value::EL3 => ExceptionLevel::SecureMonitor,
    }
}

#[inline(always)]
pub fn read_frame_pointer() -> VirtualAddress {
    let fp: usize;
    unsafe {
        core::arch::asm!("mov {}, x29", out(reg) fp);
    }
    log_debug!("fp is 0x{:x}", fp);
    VirtualAddress::new_unaligned(fp as *const _)
}

#[inline(always)]
#[cfg(target_arch = "aarch64")]
pub fn read_pc() -> *const () {
    let mut pc: *const ();
    unsafe { core::arch::asm!("adr {}, .", out(reg) pc) };
    pc
}

#[inline(always)]
#[cfg(not(target_arch = "aarch64"))]
pub fn read_pc() -> *const () {
    core::ptr::null()
}

#[inline(always)]
/// # Safety
/// The stack pointer must be valid, as well as the jumping address. From this point onwards, the
/// previous memory in the stack will no longer be accessible. Therefore references to the stack
/// will not be valid anymore.
pub unsafe fn jump_to_addr(_addr: usize, _stack_ptr: *const u8) -> ! {
    #[cfg(target_arch = "aarch64")]
    core::arch::asm!(
        "mov sp, {}",
        "dsb sy",
        "blr {}",
        in(reg) _stack_ptr,
        in(reg) _addr);

    loop {
        asm::wfi();
    }
}

pub enum StackType {
    KernelStack,
    ProcessStack,
}

impl StackType {
    #[must_use]
    pub fn current() -> Self {
        match SPSel.read_as_enum(SPSel::SP).unwrap() {
            SPSel::SP::Value::EL0 => Self::ProcessStack,
            SPSel::SP::Value::ELx => Self::KernelStack,
        }
    }
}
