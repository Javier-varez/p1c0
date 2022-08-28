use crate::print;

use tock_registers::{
    register_bitfields,
    registers::{ReadOnly, ReadWrite},
};

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
            // TODO(javier-varez): Remove hardcoded uart path and figure out where to provide this from
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
    use crate::{
        adt::AdtNode,
        drivers::DeviceRef,
        memory::{address::Address, MemoryManager},
        prelude::*,
        print,
        sync::spinlock::RwSpinLock,
    };
    use alloc::sync::Arc;

    use p1c0_macros::initcall;
    use tock_registers::interfaces::{Readable, Writeable};

    pub struct UartDriver {}

    impl super::super::Driver for UartDriver {
        fn probe(&self, dev_path: &[AdtNode]) -> crate::drivers::Result<DeviceRef> {
            let adt = crate::adt::get_adt().unwrap();
            let (device_addr, size) = adt.get_device_addr_from_nodes(dev_path, 0).unwrap();

            let mut mem_mgr = MemoryManager::instance();
            let vaddr = mem_mgr
                .map_io(dev_path.last().unwrap().get_name(), device_addr, size)
                .unwrap();

            let regs = unsafe { &*(vaddr.as_mut_ptr() as *const _) };
            let dev = Arc::new(RwSpinLock::new(Uart { regs }));

            // On success we register this device as the printer
            print::register_printer(dev.clone());
            Ok(dev)
        }
    }

    #[initcall(priority = 0)]
    fn late_uart_register_driver() {
        super::super::register_driver("uart-1,samsung", Box::new(UartDriver {})).unwrap();
    }

    pub struct Uart {
        regs: &'static UartRegs,
    }

    impl Uart {
        // TODO(javier-varez): Use interrupts for handling the UART
        fn putchar(&mut self, character: u8) {
            while self.regs.status.read(Status::TXBE) == 0 {}

            self.regs.tx.set(character as u32);
        }
    }

    impl crate::drivers::Device for Uart {}
    impl crate::drivers::interfaces::logger::Logger for Uart {}
    impl crate::print::Print for Uart {
        fn write_u8(&mut self, c: u8) -> Result<(), print::Error> {
            self.putchar(c);
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
