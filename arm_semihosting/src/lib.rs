#![cfg_attr(not(test), no_std)]
#![cfg_attr(target_arch = "arm", feature(isa_attribute))]

#[cfg_attr(target_arch = "aarch64", path = "arch/aarch64.rs")]
#[cfg_attr(target_arch = "arm", path = "arch/arm.rs")]
#[cfg_attr(any(target_arch = "x86", target_arch = "x86_64"), path = "arch/x86.rs")]
pub mod arch;

pub mod io;

use io::{OpenArgs, ReadArgs, WriteArgs};

use core::convert::From;

#[derive(Debug)]
pub enum Error {
    CouldNotReadExtensions,
    IoError(io::Error),
}

impl From<io::Error> for Error {
    fn from(io_err: io::Error) -> Self {
        Error::IoError(io_err)
    }
}

#[repr(usize)]
enum ExitReason {
    ApplicationExit = 0x20026,
}

#[repr(usize)]
enum Operation<'a> {
    Open(OpenArgs),
    Read(ReadArgs<'a>),
    Write(WriteArgs<'a>),
    ExitExtended(ExitArgs),
}

#[repr(C)]
struct ExitArgs {
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

impl<'a> Operation<'a> {
    #[inline]
    fn code(&self) -> usize {
        match *self {
            Operation::Open(_) => 0x01,
            Operation::Write(_) => 0x05,
            Operation::Read(_) => 0x06,
            Operation::ExitExtended(_) => 0x20,
        }
    }

    #[inline]
    fn args(&self) -> usize {
        match self {
            Operation::Open(args) => args.get_args(),
            Operation::Write(args) => args.get_args(),
            Operation::Read(args) => args.get_args(),
            Operation::ExitExtended(args) => args.get_args(),
        }
    }
}

struct HostResult(isize);

pub fn exit(exit_code: u32) -> ! {
    let op = Operation::ExitExtended(ExitArgs {
        sh_reason: ExitReason::ApplicationExit,
        exit_code: exit_code as usize,
    });

    arch::call_host(&op);
    unreachable!();
}

#[derive(Debug, Default)]
pub struct Extensions {
    extended_exit: bool,
    stdout_stderr: bool,
}

impl Extensions {
    pub fn supports_extended_exit(&self) -> bool {
        self.extended_exit
    }
    pub fn supports_stdout_stderr(&self) -> bool {
        self.stdout_stderr
    }
}

pub fn load_extensions() -> Result<Extensions, Error> {
    let mut extensions_file = io::open(":semihosting-features", io::AccessType::Binary)?;

    let mut buffer = [0u8; 5];
    let total_read = extensions_file.read(&mut buffer)?;

    const EXPECTED_MAGIC: [u8; 4] = [0x53, 0x48, 0x46, 0x42];

    match total_read.cmp(&4) {
        core::cmp::Ordering::Less => Err(Error::CouldNotReadExtensions),
        core::cmp::Ordering::Equal => Ok(Extensions::default()),
        core::cmp::Ordering::Greater if buffer[..4] != EXPECTED_MAGIC => {
            Err(Error::CouldNotReadExtensions)
        }
        core::cmp::Ordering::Greater => Ok(Extensions {
            extended_exit: (buffer[4] & (1 << 0)) != 0,
            stdout_stderr: (buffer[4] & (1 << 1)) != 0,
        }),
    }
}
