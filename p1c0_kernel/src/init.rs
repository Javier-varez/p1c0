use core::time::Duration;

use cortex_a::{
    asm,
    registers::{CurrentEL, CNTHCTL_EL2, CNTVOFF_EL2, ELR_EL2, HCR_EL2, SPSR_EL2, SP_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::{
    arch::{apply_rela_from_existing, exceptions, jump_to_addr, read_pc, RelaEntry},
    boot_args::BootArgs,
    chickens,
    drivers::{aic, generic_timer, uart, wdt},
    memory::{
        self,
        address::{Address, PhysicalAddress},
    },
    println,
};

/// This is the original base passed by iBoot into the kernel. Does NOT change after kernel
/// relocation.
static mut BASE: *const u8 = core::ptr::null();

fn transition_to_el1(stack_bottom: *const ()) -> ! {
    // Do not trap timer to EL2.
    CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCTEN::SET + CNTHCTL_EL2::EL1PCEN::SET);
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
    static _rela_start: u8;
    static _rela_end: u8;
    static _stack_bot: u8;
}

unsafe fn jump_to_high_kernel() -> ! {
    let new_base = PhysicalAddress::try_from_ptr(BASE)
        .and_then(|pa| pa.try_into_logical())
        .expect("Base does not have a logical address");

    let rela_start = &_rela_start as *const _ as *const RelaEntry;
    let rela_end = &_rela_end as *const _ as *const RelaEntry;
    let rela_size = rela_end.offset_from(rela_start) as usize * core::mem::size_of::<RelaEntry>();

    println!(
        "Relocating kernel to base {}, rela start {:?}, rela size {}",
        new_base, rela_start, rela_size
    );

    let high_kernel_addr =
        PhysicalAddress::from_unaligned_ptr(kernel_prelude as unsafe fn() as *const u8)
            .try_into_logical()
            .expect("The kernel prelude does not have a high kernel address");
    let high_stack = PhysicalAddress::from_unaligned_ptr(&_stack_bot as *const u8)
        .try_into_logical()
        .expect("The stack bottom does not have a high kernel address");

    // Relocate ourselves again to the correct location
    apply_rela_from_existing(BASE as usize, new_base.as_usize(), rela_start, rela_size);

    println!(
        "Jumping to relocated kernel at: {}, stack: {}, current PC {:?}",
        high_kernel_addr,
        high_stack,
        read_pc()
    );

    // From this point onwards the execution is redirected to the new kernel_prelude entrypoint.
    // We restore the initial stack using the new base address and.
    jump_to_addr(high_kernel_addr.as_usize(), high_stack.as_ptr());
}

unsafe fn kernel_prelude() {
    println!("Entering kernel prelude with PC: {:?}", read_pc());

    memory::MemoryManager::instance().late_init();

    exceptions::handling_init();
    // This services and initializes the watchdog (on first call). To avoid a reboot we should
    // periodically call this function
    wdt::service();

    let aic = aic::Aic::probe("/arm-io/aic").unwrap();
    aic::AIC.replace(aic);

    // Initialize periodic timer
    const TIMESTEP: Duration = Duration::from_millis(1);
    generic_timer::get_timer().initialize(TIMESTEP);

    kernel_main();
}

/// # Safety
///   This function must be called with the MMU off while running in EL1. It will relocate itself
unsafe extern "C" fn el1_entry() -> ! {
    memory::MemoryManager::early_init();

    // Right after initializing the MMU we need to relocate ourselves into the high_kernel_addr
    // region.
    jump_to_high_kernel();
}

#[no_mangle]
pub extern "C" fn start_rust(boot_args: &BootArgs, base: *const u8, stack_bottom: *const ()) -> ! {
    // SAFETY
    // This is safe because at this point there is only one thread running and no one has accessed
    // the boot args yet.
    unsafe { crate::boot_args::set_boot_args(boot_args) };
    unsafe { BASE = base };

    exceptions::handling_init();

    // # Safety
    //   It is safe to call probe early here since we are in a single-threaded context.
    unsafe { uart::probe_early() };

    chickens::init_cpu();

    match CurrentEL.read_as_enum(CurrentEL::EL).expect("Valid EL") {
        CurrentEL::EL::Value::EL2 => {
            transition_to_el1(stack_bottom);
        }
        CurrentEL::EL::Value::EL1 => {
            unsafe { el1_entry() };
        }
        _ => {
            panic!();
        }
    }
}
