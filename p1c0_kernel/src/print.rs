use crate::{
    collections::ring_buffer::{self, RingBuffer},
    drivers::{Dev, DeviceRef},
    init::is_kernel_relocated,
    sync::spinlock::SpinLock,
    syscall::Syscall,
};

use core::fmt::Write;

#[derive(Debug)]
pub enum Error {
    EarlyPrintFailed,
    PrintFailed,
    BufferFull,
    WriterLocked,
}

/// Marker trait to indicate this logger can be used early during the boot chain
/// (Before MMU is active)
pub trait EarlyPrint: Write {}

pub trait Print {
    fn write_str(&self, s: &str) -> Result<(), Error> {
        for character in s.bytes() {
            if character == b'\n' {
                // Implicit \r with every \n
                self.write_u8(b'\r')?;
            }
            self.write_u8(character)?;
        }
        Ok(())
    }

    fn write_u8(&self, c: u8) -> Result<(), Error>;
}

// This variable is used during early boot and therefore this cannot be wrapped in a mutex/spinlock,
// because during early boot the MMU might be off (atomics won't work) and there is no scheduler.
//
// However, given it runs in a single-threaded context it should be mostly ok.
static mut EARLY_PRINT: Option<*mut dyn EarlyPrint> = None;

static PRINT: SpinLock<Option<DeviceRef>> = SpinLock::new(None);

const BUFFER_SIZE: usize = 1024 * 256;
static BUFFER: RingBuffer<BUFFER_SIZE> = RingBuffer::new();
static LOG_WRITER: SpinLock<Option<LogWriter>> = SpinLock::new(None);

struct LogWriter<'a> {
    writer: ring_buffer::Writer<'a, BUFFER_SIZE>,
}

impl<'a> Write for LogWriter<'a> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            self.writer.push(c).map_err(|_| core::fmt::Error)?;
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) -> Result<(), Error> {
    if is_kernel_relocated() {
        let mut writer = LOG_WRITER.try_lock().map_err(|_| Error::WriterLocked)?;

        if writer.is_none() {
            let buffer_writer = BUFFER
                .split_writer()
                .expect("The buffer should not be split");
            writer.replace(LogWriter {
                writer: buffer_writer,
            });
        }

        writer
            .as_mut()
            .unwrap()
            .write_fmt(args)
            .map_err(|_| Error::BufferFull)?;
    } else {
        // We check if there is an EarlyPrint implementation and use that.

        // # Safety
        //   this should be safe given that code is single-threaded until the kernel is relocated,
        //   at which point we will no longer use the early printer.
        let early_print = unsafe { EARLY_PRINT.clone().take() };
        if let Some(printer) = early_print.map(|ptr| unsafe { &mut *ptr }) {
            printer
                .write_fmt(args)
                .map_err(|_| Error::EarlyPrintFailed)?;
        }
    }
    Ok(())
}

/// # Safety
///   This should only be called during system startup while the relocations haven't yet been done.
#[inline]
pub unsafe fn register_early_printer<T: EarlyPrint>(printer: &'static mut T) {
    EARLY_PRINT.replace(printer);
}

#[inline]
pub fn register_printer(printer: DeviceRef) {
    let mut reader = BUFFER.split_reader().expect("The buffer is already split!");

    match &*printer.lock_read() {
        Dev::Logger(_) => {}
        _ => {
            panic!("Printer must be a Dev::Logger instance");
        }
    }

    crate::thread::Builder::new()
        .name("Printer")
        .spawn(move || {
            PRINT.lock().replace(printer);
            loop {
                match reader.pop() {
                    Ok(val) => {
                        let mut lock = PRINT.lock();
                        if let Some(lock) = lock.as_mut() {
                            let mut lock = lock.lock_write();
                            match &mut *lock {
                                Dev::Logger(logger) => {
                                    logger.write_u8(val).unwrap();
                                }
                                _ => {
                                    panic!("Printer must be a Dev::Logger instance");
                                }
                            };
                        }
                    }
                    Err(ring_buffer::Error::WouldBlock) => {
                        // TODO(javier-varez): Sleep here waiting for condition to happen instead of looping
                        // At the time of this writing there is no mechanism to do this.
                        // We can at least yield to the scheduler again
                        Syscall::yield_exec();
                        continue;
                    }
                    Err(e) => {
                        panic!("Error reading from the print buffer, {:?}", e);
                    }
                }
            }
        });
}

/// # Safety
///   Only callable from a single-threaded context if the reader thread is stuck
pub unsafe fn force_flush() {
    let mut reader = BUFFER.split_reader_unchecked();
    PRINT.access_inner_without_locking(|printer| {
        printer
            .as_ref()
            .unwrap()
            .access_inner_without_locking(|printer| {
                let logger = match &mut *printer {
                    Dev::Logger(logger) => logger,
                    _ => {
                        panic!("Printer must be a Dev::Logger instance");
                    }
                };
                while let Ok(val) = reader.pop() {
                    logger.write_u8(val).unwrap();
                }
            });
    });
}
