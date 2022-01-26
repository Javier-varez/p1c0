use cortex_a::registers::CurrentEL;
use tock_registers::interfaces::Readable;

pub mod alloc;
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
pub fn read_pc() -> *const () {
    let mut pc: *const ();

    #[cfg(not(test))]
    unsafe {
        core::arch::asm!("adr {}, .", out(reg) pc)
    };

    #[cfg(test)]
    {
        pc = core::ptr::null();
    }

    pc
}
