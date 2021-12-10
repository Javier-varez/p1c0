use crate::boot_args::BootArgs;
use cortex_a::{
    asm,
    registers::{CurrentEL, CNTHCTL_EL2, CNTVOFF_EL2, ELR_EL2, HCR_EL2, SPSR_EL2, SP_EL1},
};
use tock_registers::interfaces::{Readable, Writeable};

pub mod alloc;
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

fn transition_to_el1(stack_bottom: *const ()) -> ! {
    // Do not trap timer to EL2.
    CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCTEN::CLEAR + CNTHCTL_EL2::EL1PCEN::CLEAR);
    CNTVOFF_EL2.set(0);

    // EL1 is Aarch64
    HCR_EL2.write(HCR_EL2::RW::EL1IsAarch64);

    SPSR_EL2.write(
        SPSR_EL2::D::Masked
            + SPSR_EL2::A::Masked
            + SPSR_EL2::I::Masked
            + SPSR_EL2::F::Masked
            + SPSR_EL2::M::EL1h, // We "came" from el1h
    );

    // Link register is kernel_main
    ELR_EL2.set(kernel_main as *const () as u64);

    // TODO(javier-varez): Set proper stack pointer here...
    SP_EL1.set(stack_bottom as u64);

    asm::eret();
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

extern "C" {
    pub fn kernel_main();

    static mut _arena_start: u8;
    static _arena_size: u8;
}

#[no_mangle]
pub extern "C" fn start_rust(boot_args: &BootArgs, _base: *const (), stack_bottom: *const ()) -> ! {
    // SAFETY
    // This is safe because at this point there is only one thread running and no one has accessed
    // the boot args yet.
    unsafe { crate::boot_args::set_boot_args(boot_args) };

    unsafe {
        let arena_size = (&_arena_size) as *const u8 as usize;
        let arena_start = (&mut _arena_start) as *mut u8;
        alloc::init(arena_start, arena_size);
    }

    match CurrentEL.read_as_enum(CurrentEL::EL).expect("Valid EL") {
        CurrentEL::EL::Value::EL2 => {
            transition_to_el1(stack_bottom);
        }
        CurrentEL::EL::Value::EL1 => {
            unsafe { kernel_main() };
            loop {
                asm::wfi();
            }
        }
        _ => {
            panic!();
        }
    }
}
