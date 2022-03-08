use crate::init::is_kernel_relocated;
use crate::sync::spinlock::SpinLock;
use core::fmt::Write;

#[derive(Debug)]
pub enum Error {
    PrintFailed,
    EarlyPrintFailed,
    // TODO(javier-varez): Enable the following variant when Buffering is implemented.
    // BufferFull
}

/// Marker trait to indicate this logger can be used early during the boot chain
/// (Before MMU is active)
pub trait EarlyPrint: core::fmt::Write {}

pub trait Print {
    fn write_str(&self, s: &str) -> Result<(), Error>;
}

// This variable is used during early boot and therefore this cannot be wrapped in a mutex/spinlock,
// because during early boot the MMU might be off (atomics won't work) and there is no scheduler.
//
// However, given it runs in a single-threaded context it should be mostly ok.
static mut EARLY_PRINT: Option<*mut dyn EarlyPrint> = None;

/*
 * TODO(javier-varez): This is a bit messy with the statics...
 * What we really need here is a RWSpinlock that can differentiate when we mutate the option and
 * when we just want to have a reference to the printer
 */
static PRINT: SpinLock<Option<&dyn Print>> = SpinLock::new(None);

struct LogWriter<'a> {
    printer: &'a dyn Print,
}

impl<'a> core::fmt::Write for LogWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.printer.write_str(s).map_err(|_| core::fmt::Error)
    }
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) -> Result<(), Error> {
    if is_kernel_relocated() {
        // TODO(javier-varez): Do buffered logging
        match PRINT.lock().clone() {
            Some(printer) => {
                let mut writer = LogWriter { printer };
                writer.write_fmt(args).map_err(|_| Error::PrintFailed)?;
            }
            None => {}
        }
    } else {
        // We check if there is an EarlyPrint implementation and use that.

        // # Safety
        //   this should be safe given that code is single-threaded until the kernel is relocated,
        //   at which point we will no longer use the early printer.
        let early_print = unsafe { EARLY_PRINT.clone().take() };
        match early_print.map(|ptr| unsafe { &mut *ptr }) {
            Some(printer) => {
                printer
                    .write_fmt(args)
                    .map_err(|_| Error::EarlyPrintFailed)?;
            }
            None => {}
        }
    }
    Ok(())
}

#[inline]
pub unsafe fn register_early_printer<T: EarlyPrint>(printer: &'static mut T) {
    EARLY_PRINT.replace(printer);
}

#[inline]
pub fn register_printer<T: Print>(printer: &'static T) {
    PRINT.lock().replace(printer);
}
