use core::fmt;
use cortex_a::{asm::barrier, registers::*};
use tock_registers::{
    interfaces::{Readable, Writeable},
    registers::InMemoryRegister,
};

use crate::drivers::interfaces::timer::Timer;

#[cfg(all(target_os = "none", target_arch = "aarch64", not(test)))]
use core::arch::global_asm;

// Assembly code for the exception table ane entry points
#[cfg(all(target_os = "none", target_arch = "aarch64", not(test)))]
global_asm!(include_str!("exceptions.s"));

use crate::{
    drivers::generic_timer, log_debug, log_error, log_info, syscall::syscall_handler, thread,
};

/// Wrapper structs for memory copies of registers.
#[repr(transparent)]
pub struct SpsrEL1(InMemoryRegister<u64, SPSR_EL1::Register>);

#[repr(transparent)]
pub struct EsrEL1(InMemoryRegister<u64, ESR_EL1::Register>);

impl SpsrEL1 {
    pub fn as_raw(&self) -> u64 {
        self.0.get()
    }

    pub fn from_raw(&mut self, value: u64) {
        self.0.set(value);
    }
}

/// The exception context as it is stored on the stack on exception entry.
#[repr(C)]
pub struct ExceptionContext {
    /// Exception link register. The program counter at the time the exception happened.
    pub elr_el1: u64,

    /// Saved program status.
    pub spsr_el1: SpsrEL1,

    // Exception syndrome register.
    pub esr_el1: EsrEL1,

    // Stack pointer for EL0
    pub sp_el0: u64,

    /// General Purpose Registers.
    pub gpr: [u64; 31],
}

impl Default for ExceptionContext {
    fn default() -> Self {
        Self {
            elr_el1: 0,
            spsr_el1: SpsrEL1(InMemoryRegister::new(0)),
            esr_el1: EsrEL1(InMemoryRegister::new(0)),
            sp_el0: 0,
            gpr: [0; 31],
        }
    }
}

/// Prints verbose information about the exception and then panics.
fn default_exception_handler(exc: &ExceptionContext) {
    panic!(
        "\n\nCPU Exception!\n\
        Exc level {:?}\n\
        {}",
        crate::arch::get_exception_level(),
        exc
    );
}

fn handle_fiq(e: &mut ExceptionContext) {
    let timer = generic_timer::get_timer();

    if timer.is_irq_active() {
        timer.handle_irq();

        // Run scheduler and maybe do context switch
        thread::run_scheduler(e);

        // FIXME(javier-varez): This is a workaround for m1n1 HV. m1n1 triggers a Virtual FIQ that
        // p1c0 handles when the timer expires, but it doesn't get notified by writes to TVAL or CTL
        // timer registers.
        //
        // Since it doesn't listen to those register accesses it simply checks when to disable the FIQ
        // by polling when the HV happens to run again.
        //
        // The problem is that it might take a while until it checks again and then the FIQ remains
        // active for no good reason. Since p1c0 doesn't know what caused the FIQ, it calls the
        // default handler and ends up crashing.
        //
        // PMCR0 is trapped by the HV, so this causes m1n1 HV to check again and synchronously
        // disable the Virtual FIQ.
        crate::registers::SYS_IMPL_APL_PMCR0.get();
        return;
    }

    log_info!("FIQ");
    if let Some(aic) = unsafe { crate::drivers::aic::AIC.as_mut() } {
        if let Some((die, number, r#type)) = aic.get_current_irq() {
            log_debug!("Irq die {}", die);
            log_debug!("Irq number {}", number);
            log_debug!("Irq type {:?}", r#type);
        }
    }
    default_exception_handler(e);
}

enum ExceptionOrigin {
    SameELAndStack,
    SameELStackFromEL0,
    LowerAarch64EL,
}

unsafe fn handle_synchronous(e: &mut ExceptionContext, origin: ExceptionOrigin) {
    match e.esr_el1.exception_class() {
        Some(ESR_EL1::EC::Value::SVC64) => {
            syscall_handler(e.esr_el1.instruction_specific_syndrome(), e);
        }
        _ => {
            match origin {
                ExceptionOrigin::SameELStackFromEL0 => {
                    log_info!("Synchronous exception from EL1 with EL0 stack");
                    default_exception_handler(e);
                }
                ExceptionOrigin::SameELAndStack => {
                    log_info!("Synchronous exception from EL1");
                    default_exception_handler(e);
                }
                ExceptionOrigin::LowerAarch64EL => {
                    log_info!("Synchronous exception from EL0");
                    // Get userspace process and kill it.
                    // Some exceptions should be handled in the future (like accesses to
                    // unmapped memory regions)
                    log_error!(
                        "\n\nCPU Exception!\n\
                        Exc level {:?}\n\
                        {}",
                        crate::arch::get_exception_level(),
                        e
                    );

                    crate::process::kill_current_process(e).unwrap();
                    return;
                }
            }
        }
    }
}

#[no_mangle]
unsafe extern "C" fn current_el0_synchronous(e: &mut ExceptionContext) {
    handle_synchronous(e, ExceptionOrigin::SameELStackFromEL0);
}

#[no_mangle]
unsafe extern "C" fn current_el0_irq(e: &mut ExceptionContext) {
    log_info!("IRQ from EL0 stack");

    if let Some(aic) = &mut crate::drivers::aic::AIC {
        if let Some((die, number, r#type)) = aic.get_current_irq() {
            log_debug!("Irq die {}", die);
            log_debug!("Irq number {}", number);
            log_debug!("Irq type {:?}", r#type);
        }
    }
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn current_el0_fiq(e: &mut ExceptionContext) {
    handle_fiq(e);
}

#[no_mangle]
unsafe extern "C" fn current_el0_serror(e: &mut ExceptionContext) {
    log_info!("Serror exception from EL0 stack");
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn current_elx_synchronous(e: &mut ExceptionContext) {
    handle_synchronous(e, ExceptionOrigin::SameELAndStack);
}

#[no_mangle]
unsafe extern "C" fn current_elx_fiq(e: &mut ExceptionContext) {
    handle_fiq(e);
}

#[no_mangle]
unsafe extern "C" fn current_elx_irq(e: &mut ExceptionContext) {
    log_info!("IRQ");

    if let Some(aic) = &mut crate::drivers::aic::AIC {
        if let Some((die, number, r#type)) = aic.get_current_irq() {
            log_debug!("Irq die {}", die);
            log_debug!("Irq number {}", number);
            log_debug!("Irq type {:?}", r#type);
        }
    }

    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn current_elx_serror(e: &mut ExceptionContext) {
    log_info!("Serror exception");
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch64_synchronous(e: &mut ExceptionContext) {
    handle_synchronous(e, ExceptionOrigin::LowerAarch64EL);
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch64_irq(e: &mut ExceptionContext) {
    log_info!(
        "lower_el_aarch64_irq: {:?}",
        crate::arch::get_exception_level()
    );
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch64_fiq(e: &mut ExceptionContext) {
    handle_fiq(e)
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch64_serror(e: &mut ExceptionContext) {
    log_info!(
        "lower_el_aarch64_serror: {:?}",
        crate::arch::get_exception_level()
    );
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch32_synchronous(_e: &mut ExceptionContext) {
    panic!(
        "lower_el_aarch32_synchronous: {:?}. This should not happen!",
        crate::arch::get_exception_level()
    );
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch32_irq(_e: &mut ExceptionContext) {
    panic!(
        "lower_el_aarch32_irq: {:?}. This should not happen!",
        crate::arch::get_exception_level()
    );
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch32_fiq(_e: &mut ExceptionContext) {
    panic!(
        "lower_el_aarch32_fiq: {:?}. This should not happen!",
        crate::arch::get_exception_level()
    );
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch32_serror(_e: &mut ExceptionContext) {
    panic!(
        "lower_el_aarch32_serror: {:?}. This should not happen!",
        crate::arch::get_exception_level()
    );
}

/// Human readable SPSR_EL1.
#[rustfmt::skip]
impl fmt::Display for SpsrEL1 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Raw value.
        writeln!(f, "SPSR_EL1: {:#010x}", self.0.get())?;

        let to_flag_str = |x| -> _ {
            if x { "Set" } else { "Not set" }
        };

        writeln!(f, "      Flags:")?;
        writeln!(f, "            Negative (N): {}", to_flag_str(self.0.is_set(SPSR_EL1::N)))?;
        writeln!(f, "            Zero     (Z): {}", to_flag_str(self.0.is_set(SPSR_EL1::Z)))?;
        writeln!(f, "            Carry    (C): {}", to_flag_str(self.0.is_set(SPSR_EL1::C)))?;
        writeln!(f, "            Overflow (V): {}", to_flag_str(self.0.is_set(SPSR_EL1::V)))?;

        let to_mask_str = |x| -> _ {
            if x { "Masked" } else { "Unmasked" }
        };

        writeln!(f, "      Exception handling state:")?;
        writeln!(f, "            Debug  (D): {}", to_mask_str(self.0.is_set(SPSR_EL1::D)))?;
        writeln!(f, "            SError (A): {}", to_mask_str(self.0.is_set(SPSR_EL1::A)))?;
        writeln!(f, "            IRQ    (I): {}", to_mask_str(self.0.is_set(SPSR_EL1::I)))?;
        writeln!(f, "            FIQ    (F): {}", to_mask_str(self.0.is_set(SPSR_EL1::F)))?;

        write!(f, "      Illegal Execution State (IL): {}",
               to_flag_str(self.0.is_set(SPSR_EL1::IL))
        )
    }
}

impl EsrEL1 {
    #[inline(always)]
    fn exception_class(&self) -> Option<ESR_EL1::EC::Value> {
        self.0.read_as_enum(ESR_EL1::EC)
    }

    #[inline(always)]
    fn instruction_specific_syndrome(&self) -> u32 {
        self.0.read(ESR_EL1::ISS) as u32
    }
}

#[rustfmt::skip]
impl fmt::Display for EsrEL1 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Raw print of whole register.
        writeln!(f, "ESR_EL1: {:#010x}", self.0.get())?;

        // Raw print of exception class.
        write!(f, "      Exception Class         (EC) : {:#x}", self.0.read(ESR_EL1::EC))?;

        // Exception class.
        let ec_translation = match self.exception_class() {
            Some(ESR_EL1::EC::Value::DataAbortCurrentEL) => "Data Abort, current EL",
            Some(ESR_EL1::EC::Value::InstrAbortCurrentEL) => "Instruction Abort, current EL",
            Some(ESR_EL1::EC::Value::DataAbortLowerEL) => "Data Abort, lower EL",
            Some(ESR_EL1::EC::Value::InstrAbortLowerEL) => "Instruction Abort, lower EL",
            Some(ESR_EL1::EC::Value::SVC64) => "SVC Call",
            Some(ESR_EL1::EC::Value::SVC32) => "SVC Call (32-bit)",
            _ => "N/A",
        };
        writeln!(f, " - {}", ec_translation)?;

        // Raw print of instruction specific syndrome.
        write!(f, "      Instr Specific Syndrome (ISS): {:#x}", self.0.read(ESR_EL1::ISS))
    }
}

impl ExceptionContext {
    #[inline(always)]
    fn exception_class(&self) -> Option<ESR_EL1::EC::Value> {
        self.esr_el1.exception_class()
    }

    #[inline(always)]
    fn fault_address_valid(&self) -> bool {
        use ESR_EL1::EC::Value::*;

        match self.exception_class() {
            None => false,
            Some(ec) => matches!(
                ec,
                InstrAbortLowerEL
                    | InstrAbortCurrentEL
                    | PCAlignmentFault
                    | DataAbortLowerEL
                    | DataAbortCurrentEL
                    | WatchpointLowerEL
                    | WatchpointCurrentEL
            ),
        }
    }
}

/// Human readable print of the exception context.
impl fmt::Display for ExceptionContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", self.esr_el1)?;

        if self.fault_address_valid() {
            writeln!(f, "FAR_EL1: {:#018x}", FAR_EL1.get() as usize)?;
        }

        writeln!(f, "{}", self.spsr_el1)?;
        writeln!(f, "ELR_EL1: {:#018x}", self.elr_el1)?;
        writeln!(f)?;
        writeln!(f, "General purpose register:")?;

        #[rustfmt::skip]
            let alternating = |x| -> _ {
            if x % 2 == 0 { "   " } else { "\n" }
        };

        // Print two registers per line.
        for (i, reg) in self.gpr.iter().enumerate() {
            write!(f, "      x{: <2}: {: >#018x}{}", i, reg, alternating(i))?;
        }
        write!(f, "")
    }
}

extern "C" {
    pub static __exception_vector_start: u8;
    pub static __el2_exception_vector_start: u8;
}

/// Init exception handling by setting the exception vector base address register.
pub fn handling_init() {
    #[cfg(target_os = "none")]
    let vectors = unsafe { &__exception_vector_start as *const _ };

    #[cfg(not(target_os = "none"))]
    let vectors = 0;

    VBAR_EL1.set(vectors as u64);
    // Force VBAR update to complete before next instruction.
    unsafe { barrier::isb(barrier::SY) };

    if matches!(
        CurrentEL.read_as_enum(CurrentEL::EL),
        Some(CurrentEL::EL::Value::EL2)
    ) {
        HCR_EL2.write(
            HCR_EL2::RW::EL1IsAarch64
            // These settings would make EL2 work just like an OS and also trap any exceptions
            // from EL1 to EL2. EL1 cannot be used with them.
            //
            // + HCR_EL2::API::NoTrapPointerAuthInstToEl2
            // + HCR_EL2::APK::NoTrapPointerAuthKeyRegsToEl2
            // + HCR_EL2::TEA::RouteSyncExtAborts
            // + HCR_EL2::E2H::EnableOsAtEl2
            // + HCR_EL2::TGE::TrapGeneralExceptions
            // + HCR_EL2::AMO::SET
            // + HCR_EL2::IMO::SET
            // + HCR_EL2::FMO::SET,
        );

        // Force HCR update to complete before next instruction.
        unsafe { barrier::isb(barrier::SY) };

        #[cfg(target_os = "none")]
        let vectors = unsafe { &__el2_exception_vector_start as *const _ };

        #[cfg(not(target_os = "none"))]
        let vectors = 0;

        VBAR_EL2.set(vectors as u64);

        // Force VBAR update to complete before next instruction.
        unsafe { barrier::isb(barrier::SY) };
    }
}

/// This simulates a return from an exception with the given context. It can be used whenever
/// you need to immediately return from an exception without waiting for the handler to finish.
/// It is also useful to return from exceptions that never happened (like transitioning to another
/// thread, or starting the scheduler)
pub fn return_from_exception(cx: ExceptionContext) -> ! {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        barrier::dsb(barrier::SY);
        core::arch::asm!(
        "ldp x0, x1, [x30, #0x00]",
        "ldp x2, x3, [x30, #0x10]",
        "msr ELR_EL1,  x0",
        "msr SPSR_EL1, x1",
        "msr SP_EL0, x3",
        "ldp x0,  x1,  [x30, #0x20]",
        "ldp x2,  x3,  [x30, #0x30]",
        "ldp x4,  x5,  [x30, #0x40]",
        "ldp x6,  x7,  [x30, #0x50]",
        "ldp x8,  x9,  [x30, #0x60]",
        "ldp x10, x11, [x30, #0x70]",
        "ldp x12, x13, [x30, #0x80]",
        "ldp x14, x15, [x30, #0x90]",
        "ldp x16, x17, [x30, #0xA0]",
        "ldp x18, x19, [x30, #0xB0]",
        "ldp x20, x21, [x30, #0xC0]",
        "ldp x22, x23, [x30, #0xD0]",
        "ldp x24, x25, [x30, #0xE0]",
        "ldp x26, x27, [x30, #0xF0]",
        "ldp x28, x29, [x30, #0x100]",
        "ldr x30, [x30, #0x110]",
        "eret",
        in("x30") (&cx) as *const _
        );
    }
    unreachable!();
}
