use core::fmt::{self, Write};
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
    registers::{ReadOnly, ReadWrite},
};

use crate::memory::address::Address;

// Defines bitfields for the UART registers
register_bitfields![u32,
    /// Defines the status register bitfield for the UART
    Status [
        /// Whether the current transfer buffer is empty or not
        TXBE OFFSET(1) NUMBITS(1) [],
    ],
];

static mut EARLY_UART: Option<EarlyUart> = None;

#[repr(C)]
struct UartRegs {
    reserved1: [u32; 4],
    status: ReadOnly<u32, Status::Register>,
    reserved2: [u32; 3],
    tx: ReadWrite<u32>,
}

struct EarlyUart {
    regs: *mut UartRegs,
}

impl EarlyUart {
    fn new() -> Self {
        let adt = crate::adt::get_adt().unwrap();
        let (device_addr, _) = adt.get_device_addr("/arm-io/uart0", 0).unwrap();
        let regs = device_addr.as_mut_ptr() as *mut _;
        Self { regs }
    }

    fn regs(&mut self) -> &'static UartRegs {
        unsafe { &mut (*self.regs) }
    }

    fn putchar(&mut self, character: u8) {
        while self.regs().status.read(Status::TXBE) == 0 {}

        self.regs().tx.set(character as u32);
    }
}

impl fmt::Write for EarlyUart {
    fn write_str(&mut self, s: &str) -> Result<(), fmt::Error> {
        for character in s.bytes() {
            if character == b'\n' {
                // Implicit \r with every \n
                self.putchar(b'\r');
            }
            self.putchar(character);
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    let uart = unsafe { &mut EARLY_UART };
    if let Some(uart) = uart {
        uart.write_fmt(args).expect("Printing to uart failed");
    }
}

/// # Safety
///   This should only be called during system startup while the relocations haven't yet been done.
pub unsafe fn probe_early() {
    let uart = &mut EARLY_UART;
    uart.replace(EarlyUart::new());
}
