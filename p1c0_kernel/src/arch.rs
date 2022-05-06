use crate::log_debug;
use crate::memory::address::VirtualAddress;
use cortex_a::registers::{CurrentEL, SPSel};
use tock_registers::interfaces::Readable;

pub mod cache;
pub mod exceptions;
pub mod mmu;

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
