use crate::{
    adt,
    arch::{exceptions, read_pc},
    backtrace,
    boot_args::BootArgs,
    chickens, drivers,
    drivers::{aic, generic_timer, interfaces::timer::Timer, uart},
    memory::{
        self,
        address::{Address, PhysicalAddress, VirtualAddress},
        map,
    },
    prelude::*,
    registers::CPACR,
};

use p1c0_macros::initcall;

use core::time::Duration;

use cortex_a::{
    asm,
    registers::{CurrentEL, CNTHCTL_EL2, CNTVOFF_EL2, ELR_EL2, HCR_EL2, SPSR_EL2, SP_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

#[repr(C)]
struct RelaEntry {
    offset: usize,
    ty: usize,
    addend: usize,
}

/// This is the original base passed by iBoot into the kernel. Does NOT change after kernel
/// relocation.
static mut BASE: *const u8 = core::ptr::null();

static mut RELOCATION_DONE: bool = false;

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
    fn kernel_main();
    static _rela_start: u8;
    static _rela_end: u8;
    static _stack_bot: u8;

    // # SAFETY: This function assumes the new address is in high memory!
    fn relocate_and_jump_to_relocated_kernel(
        old_base: usize,
        new_base: usize,
        rela_start: *const RelaEntry,
        rela_end: *const RelaEntry,
        high_kernel_addr: usize,
        high_stack_addr: usize,
    ) -> !;
}

unsafe fn jump_to_high_kernel() -> ! {
    let new_base = PhysicalAddress::try_from_ptr(BASE)
        .and_then(|pa| pa.try_into_logical())
        .expect("Base does not have a logical address");

    let rela_start = &_rela_start as *const _ as *const RelaEntry;
    let rela_end = &_rela_end as *const _ as *const RelaEntry;
    let rela_size = rela_end.offset_from(rela_start) as usize * core::mem::size_of::<RelaEntry>();

    log_info!(
        "Relocating kernel to base {}, rela start {:?}, rela size {}",
        new_base,
        rela_start,
        rela_size
    );

    let high_kernel_addr =
        PhysicalAddress::from_unaligned_ptr(kernel_prelude as unsafe fn() as *const u8)
            .try_into_logical()
            .expect("The kernel prelude does not have a high kernel address");
    let high_stack = PhysicalAddress::from_unaligned_ptr(&_stack_bot as *const u8)
        .try_into_logical()
        .expect("The stack bottom does not have a high kernel address");

    // Relocate ourselves again to the correct location
    // From this point onwards the execution is redirected to the new kernel_prelude entrypoint.
    // We restore the initial stack using the new base address and.
    relocate_and_jump_to_relocated_kernel(
        BASE as usize,
        new_base.as_usize(),
        rela_start,
        rela_end,
        high_kernel_addr.as_usize(),
        high_stack.as_usize(),
    );
}

unsafe fn kernel_prelude() {
    let new_base = PhysicalAddress::try_from_ptr(BASE)
        .and_then(|pa| pa.try_into_logical())
        .expect("Base does not have a logical address");

    // Store the new base as logical address from now on, since it did change
    BASE = new_base.as_ptr();

    // At this point the Kernel is relocated and the initial boot process is done.
    // We set this flag to let the kernel know that it can use regular memory management
    // from now onwards.
    RELOCATION_DONE = true;
    log_info!("Entering kernel prelude with PC: {:?}", read_pc());

    // Enable FPU usage both in EL1 and EL0
    CPACR.modify(CPACR::FPEN::Enable);

    memory::MemoryManager::instance().late_init();

    exceptions::handling_init();

    let aic = aic::Aic::probe("/arm-io/aic").unwrap();
    aic::AIC.replace(aic);

    // Initialize periodic timer
    const TIMESTEP: Duration = Duration::from_millis(1);
    generic_timer::get_timer().initialize(TIMESTEP);

    // Invoke all initcalls functions
    run_initcalls();

    probe_devices();

    kernel_main();
}

fn probe_subdevices<const SIZE: usize>(devs: &mut heapless::Vec<adt::AdtNode, SIZE>) {
    let parent = devs.last().unwrap().clone();
    for subdevices in parent.child_iter() {
        devs.push(subdevices).expect("Exceeded recursion size");
        match drivers::probe_device(devs) {
            Ok(_) => {}
            Err(drivers::Error::DeviceSpecificError(dev_error)) => {
                log_warning!("Unable to probe device. Error: {:?}", dev_error);
            }
            Err(_) => {}
        }
        probe_subdevices(devs);
        devs.pop();
    }
}

fn probe_devices() {
    let adt = adt::get_adt().unwrap();
    let mut devs: heapless::Vec<adt::AdtNode, 8> = adt.path_iter("/arm-io").collect();
    probe_subdevices(&mut devs);
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
    // Warning: Be very careful of the work that is done at this stage. At this point in time the
    // kernel is about to be relocated and doesn't have an enabled MMU. What this means is that most
    // of the operations will not work or will not be compatible (e.g.: addresses) with the
    // relocated kernel.

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

#[inline]
pub fn is_kernel_relocated() -> bool {
    // This is only written during startup when interrupts are not enabled. Therefore it is safe to
    // read before booted (because it is written and read from the same thread) and afterwards
    // (because it never changes again).
    unsafe { RELOCATION_DONE }
}

/// Initcalls are expected to be called after relocation before the kernel starts parsing the ADT
/// and probing devices. This gives drivers a chance to register themselves and later be used for
/// probing devices.
///
/// # Safety
/// This function should be called in a single-threaded context when relocations have been
/// completed.
unsafe fn run_initcalls() {
    extern "C" {
        static _initcall_start: extern "C" fn();
        static _initcall_end: extern "C" fn();
    }

    let start = &_initcall_start as *const extern "C" fn();
    let end = &_initcall_end as *const extern "C" fn();
    let size = end.offset_from(start);

    let initcalls = core::slice::from_raw_parts(start, size as usize);

    for initcall in initcalls {
        initcall();
    }
}

pub(crate) fn get_base() -> VirtualAddress {
    VirtualAddress::new_unaligned(unsafe { BASE })
}

// This might contain multiple payloads appended to the binary after it has been generated
#[initcall(priority = 4)]
fn parse_payload() {
    let section = map::KernelSection::from_id(map::KernelSectionId::Payload);
    let payload_slice =
        unsafe { core::slice::from_raw_parts(section.la().as_ptr(), section.size_bytes()) };

    let mut offset = 0;
    loop {
        // Try to identify payload at current offset
        if let Ok(size) = backtrace::ksyms::parse(&payload_slice[offset..]) {
            offset += size;
            continue;
        }

        // No valid payload found, stopping now
        break;
    }
}
