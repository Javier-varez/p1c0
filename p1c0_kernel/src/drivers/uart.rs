use tock_registers::{
    register_bitfields,
    registers::{ReadOnly, ReadWrite},
};

use crate::{print, println};

// Defines bitfields for the UART registers
register_bitfields![u32,
    /// Defines the status register bitfield for the UART
    Status [
        /// Whether the current transfer buffer is empty or not
        TXBE OFFSET(1) NUMBITS(1) [],
    ],
];

#[repr(C)]
struct UartRegs {
    reserved1: [u32; 4],
    status: ReadOnly<u32, Status::Register>,
    reserved2: [u32; 3],
    tx: ReadWrite<u32>,
}

mod early_uart {
    use super::{Status, UartRegs};
    use crate::memory::address::Address;
    use crate::print::EarlyPrint;
    use tock_registers::interfaces::{Readable, Writeable};

    use core::fmt;

    pub static mut EARLY_UART: Option<EarlyUart> = None;

    pub struct EarlyUart {
        regs: *mut UartRegs,
    }

    impl EarlyUart {
        pub(super) fn new() -> Self {
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

    impl EarlyPrint for EarlyUart {}
}

mod late_uart {
    use super::{Status, UartRegs};
    use crate::memory::address::Address;
    use crate::memory::MemoryManager;
    use crate::print::{self, Print};
    use crate::sync::spinlock::SpinLock;
    use tock_registers::interfaces::{Readable, Writeable};

    pub static LATE_UART: SpinLock<Option<Uart>> = SpinLock::new(None);

    pub struct Uart {
        regs: *mut UartRegs,
    }

    impl Uart {
        pub(super) fn new() -> Self {
            let adt = crate::adt::get_adt().unwrap();
            let (device_addr, size) = adt.get_device_addr("/arm-io/uart0", 0).unwrap();

            let mut mem_mgr = MemoryManager::instance();
            let vaddr = mem_mgr.map_io("Uart regs", device_addr, size).unwrap();

            let regs = vaddr.as_mut_ptr() as *mut _;
            Self { regs }
        }

        fn regs(&mut self) -> &'static UartRegs {
            unsafe { &mut (*self.regs) }
        }

        // TODO(javier-varez): Use interrupts for handling the UART
        fn putchar(&mut self, character: u8) {
            while self.regs().status.read(Status::TXBE) == 0 {}

            self.regs().tx.set(character as u32);
        }
    }

    // We really should do better than this next time tbh
    impl Print for SpinLock<Option<Uart>> {
        fn write_str(&self, s: &str) -> Result<(), print::Error> {
            let mut lock = self.lock();
            let uart = lock.as_mut().ok_or(print::Error::PrintFailed)?;
            for character in s.bytes() {
                if character == b'\n' {
                    // Implicit \r with every \n
                    uart.putchar(b'\r');
                }
                uart.putchar(character);
            }
            drop(lock);
            Ok(())
        }
    }
}

/// # Safety
///   This should only be called during system startup while the relocations haven't yet been done.
pub unsafe fn probe_early() {
    let uart = &mut early_uart::EARLY_UART;
    uart.replace(early_uart::EarlyUart::new());

    print::register_early_printer(uart.as_mut().unwrap());
}

pub fn probe_late() {
    println!("Late uart probe");
    late_uart::LATE_UART.lock().replace(late_uart::Uart::new());
    print::register_printer(&late_uart::LATE_UART);
}
