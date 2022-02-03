use core::fmt;
use cortex_a::{asm::barrier, registers::*};
use tock_registers::{
    interfaces::{Readable, Writeable},
    registers::InMemoryRegister,
};

#[cfg(all(target_arch = "aarch64", not(test)))]
use core::arch::global_asm;

// Assembly code for the exception table ane entry points
#[cfg(all(target_arch = "aarch64", not(test)))]
global_asm!(include_str!("exceptions.s"));

/// Wrapper structs for memory copies of registers.
#[repr(transparent)]
struct SpsrEL1(InMemoryRegister<u64, SPSR_EL1::Register>);
struct EsrEL1(InMemoryRegister<u64, ESR_EL1::Register>);

/// The exception context as it is stored on the stack on exception entry.
#[repr(C)]
struct ExceptionContext {
    /// Exception link register. The program counter at the time the exception happened.
    elr_el1: u64,

    /// Saved program status.
    spsr_el1: SpsrEL1,

    // Exception syndrome register.
    esr_el1: EsrEL1,

    /// General Purpose Registers.
    gpr: [u64; 31],
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

#[no_mangle]
unsafe extern "C" fn current_el0_synchronous(_e: &mut ExceptionContext) {
    panic!("Should not be here. Use of SP_EL0 in EL1 is not supported.")
}

#[no_mangle]
unsafe extern "C" fn current_el0_irq(_e: &mut ExceptionContext) {
    panic!("Should not be here. Use of SP_EL0 in EL1 is not supported.")
}

#[no_mangle]
unsafe extern "C" fn current_el0_serror(_e: &mut ExceptionContext) {
    panic!("Should not be here. Use of SP_EL0 in EL1 is not supported.")
}

#[no_mangle]
unsafe extern "C" fn current_elx_synchronous(e: &mut ExceptionContext) {
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn current_elx_irq(e: &mut ExceptionContext) {
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn current_elx_serror(e: &mut ExceptionContext) {
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch64_synchronous(e: &mut ExceptionContext) {
    crate::println!(
        "lower_el_aarch64_synchronous: {:?}",
        crate::arch::get_exception_level()
    );
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch64_irq(e: &mut ExceptionContext) {
    crate::println!(
        "lower_el_aarch64_irq: {:?}",
        crate::arch::get_exception_level()
    );
    default_exception_handler(e);
}

#[no_mangle]
unsafe extern "C" fn lower_el_aarch64_serror(e: &mut ExceptionContext) {
    crate::println!(
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
    let vectors = unsafe { &__exception_vector_start as *const _ };
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

        let vectors = unsafe { &__el2_exception_vector_start as *const _ };
        VBAR_EL12.set(vectors as u64);

        // Force VBAR update to complete before next instruction.
        unsafe { barrier::isb(barrier::SY) };
    }
}