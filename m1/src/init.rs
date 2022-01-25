use cortex_a::{
    asm,
    registers::{CurrentEL, CNTHCTL_EL2, CNTVOFF_EL2, ELR_EL2, HCR_EL2, SPSR_EL2, SP_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::{
    arch::{alloc, exceptions, mmu},
    boot_args::BootArgs,
    chickens, uart, wdt,
};

fn transition_to_el1(stack_bottom: *const ()) -> ! {
    // Do not trap timer to EL2.
    CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCTEN::CLEAR + CNTHCTL_EL2::EL1PCEN::CLEAR);
    CNTVOFF_EL2.set(0);

    // EL1 is Aarch64
    HCR_EL2.modify(HCR_EL2::RW::EL1IsAarch64);

    SPSR_EL2.write(
        SPSR_EL2::D::Masked
            + SPSR_EL2::A::Masked
            + SPSR_EL2::I::Masked
            + SPSR_EL2::F::Masked
            + SPSR_EL2::M::EL1h, // We "came" from el1h
    );

    // Link register is el1_entry
    ELR_EL2.set(el1_entry as *const () as u64);

    // TODO(javier-varez): Set proper stack pointer here...
    SP_EL1.set(stack_bottom as u64);

    asm::eret();
}

extern "C" {
    pub fn kernel_main();

    static mut _arena_start: u8;
    static _arena_size: u8;
}

static mut BASE: *const u8 = core::ptr::null();
///
/// # Safety
///   This function must be called with the MMU off and exceptions masked while running in EL1.
pub unsafe extern "C" fn el1_entry() {
    mmu::initialize();

    let arena_size = (&_arena_size) as *const u8 as usize;
    let arena_start = (&mut _arena_start) as *mut u8;
    alloc::init(arena_start, arena_size);

    kernel_main();
}

#[no_mangle]
pub extern "C" fn start_rust(boot_args: &BootArgs, base: *const u8, stack_bottom: *const ()) -> ! {
    // SAFETY
    // This is safe because at this point there is only one thread running and no one has accessed
    // the boot args yet.
    unsafe { crate::boot_args::set_boot_args(boot_args) };
    unsafe { BASE = base };

    exceptions::handling_init();
    uart::initialize();

    chickens::init_cpu();

    // This services and initializes the watchdog (on first call). To avoid a reboot we should
    // periodically call this function
    wdt::service();

    match CurrentEL.read_as_enum(CurrentEL::EL).expect("Valid EL") {
        CurrentEL::EL::Value::EL2 => {
            transition_to_el1(stack_bottom);
        }
        CurrentEL::EL::Value::EL1 => {
            unsafe { el1_entry() };
            loop {
                asm::wfi();
            }
        }
        _ => {
            panic!();
        }
    }
}
