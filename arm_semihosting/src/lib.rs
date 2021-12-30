#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_arch = "arm", feature(isa_attribute))]

#[cfg_attr(target_arch = "aarch64", path = "arch/aarch64.rs")]
#[cfg_attr(target_arch = "arm", path = "arch/arm.rs")]
#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), path = "arch/x86.rs")]
pub mod arch;

#[repr(usize)]
pub enum ExitReason {
    ApplicationExit = 0x20026,
    InternalError = 0x20024,
}

#[repr(usize)]
pub enum Operation {
    SysExit(ExitArgs),
    SysExitExtended(ExitArgs),
}

#[repr(C)]
pub struct ExitArgs {
    sh_reason: ExitReason,
    exit_code: usize,
}

trait PointerArgs {
    #[inline]
    fn get_args(&self) -> usize {
        self as *const _ as *const () as usize
    }
}

impl PointerArgs for ExitArgs {}

impl Operation {
    #[inline]
    fn code(&self) -> usize {
        match *self {
            Operation::SysExit(_) => 0x18,
            Operation::SysExitExtended(_) => 0x20,
        }
    }

    #[inline]
    fn args(&self) -> usize {
        match self {
            Operation::SysExit(args) => args.get_args(),
            Operation::SysExitExtended(args) => args.get_args(),
        }
    }
}

pub struct HostResult(usize);

pub fn exit(exit_code: u32) -> ! {
    let op = Operation::SysExitExtended(ExitArgs {
        sh_reason: ExitReason::ApplicationExit,
        exit_code: exit_code as usize,
    });

    arch::call_host(&op);
    unreachable!();
}
