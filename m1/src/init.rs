use cortex_a::{
    asm,
    registers::{CurrentEL, CNTHCTL_EL2, CNTVOFF_EL2, ELR_EL2, HCR_EL2, SPSR_EL2, SP_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::{
    arch::{alloc, apply_rela, exceptions, jump_to_addr, mmu, read_pc, RelaEntry},
    boot_args::BootArgs,
    chickens, println, uart, wdt, KERNEL_LOGICAL_BASE,
};

/// This is the original base passed by iBoot into the kernel. Does NOT change after kernel
/// relocation.
static mut BASE: *const u8 = core::ptr::null();

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
    static _rela_start: u8;
    static _rela_end: u8;
}

unsafe fn jump_to_high_kernel() -> ! {
    let new_base = BASE as usize + KERNEL_LOGICAL_BASE as usize;
    let rela_start = &_rela_start as *const _ as *const RelaEntry;
    let rela_end = &_rela_end as *const _ as *const RelaEntry;
    let rela_size = rela_end.offset_from(rela_start) as usize * core::mem::size_of::<RelaEntry>();

    println!(
        "Relocating kernel to base 0x{:x}, rela start {:?}, rela size {}",
        new_base, rela_start, rela_size
    );

    // Relocate ourselves again to the correct location
    apply_rela(new_base, rela_start, rela_size);

    let high_kernel_addr = kernel_prelude as unsafe fn() as usize + KERNEL_LOGICAL_BASE;

    println!(
        "Jumping to relocated kernel at: 0x{:x}, current PC {:?}",
        high_kernel_addr,
        read_pc()
    );
    jump_to_addr(high_kernel_addr)
}

unsafe fn kernel_prelude() {
    println!("Entering kernel prelude with PC: {:?}", read_pc());

    let arena_size = (&_arena_size) as *const u8 as usize;
    let arena_start = (&mut _arena_start) as *mut u8;

    alloc::init(arena_start, arena_size);

    // This services and initializes the watchdog (on first call). To avoid a reboot we should
    // periodically call this function
    wdt::service();

    kernel_main();
}

/// # Safety
///   This function must be called with the MMU off and exceptions masked while running in EL1.
pub unsafe extern "C" fn el1_entry() {
    mmu::initialize();
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
    uart::initialize();

    chickens::init_cpu();

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
