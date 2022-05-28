use crate::{memory::address::Address, prelude::*, sync::spinlock::RwSpinLock, syscall, thread};

use p1c0_macros::initcall;

use tock_registers::{interfaces::Writeable, register_bitfields, registers::ReadWrite};

const COMPATIBLE: &str = "wdt,t6000";

// Defines bitfields for the WDT registers
register_bitfields![u32,
    /// Controls the state of the watchdog
    Control [
        ENABLE OFFSET(2) NUMBITS(1) [],
    ],
];

#[repr(C)]
struct WdtRegs {
    reserved1: [u32; 4],
    count: ReadWrite<u32>,
    alarm: ReadWrite<u32>,
    reserved2: u32,
    control: ReadWrite<u32, Control::Register>,
}

pub struct Wdt {
    regs: &'static WdtRegs,
}

// The watchdog seems to be running at 24 MHz by default.
// The programming sequence is:
//   * Reset the watchdog count to 0
//   * Write an alarm count. The system will be restarted when it is hit
//   * Enable the watchdog by writing the control register enable bit

impl Wdt {
    const FREQ_KHZ: u32 = 24_000;

    fn service(&self) {
        self.regs.count.set(0);
    }
}

impl super::Device for Wdt {}

struct WdtDriver {}

impl super::Driver for WdtDriver {
    fn probe(&self, dev_path: &[crate::adt::AdtNode]) -> super::Result<super::DeviceRef> {
        let adt = crate::adt::get_adt().unwrap();
        let (pa, size) = adt.get_device_addr_from_nodes(dev_path, 0).unwrap();

        let name = dev_path.last().unwrap().get_name();

        let va = crate::memory::MemoryManager::instance()
            .map_io(name, pa, size)
            .unwrap();

        let regs = unsafe { &*(va.as_mut_ptr() as *mut WdtRegs) };

        const TIMEOUT_MS: u32 = 5_000;
        regs.count.set(0);
        regs.alarm.set(Wdt::FREQ_KHZ * TIMEOUT_MS);
        regs.control.write(Control::ENABLE::SET);

        // We create a thread and service the watchdog there. If the OS halts the thread would not run, rebooting the device
        let dev = Arc::new(RwSpinLock::new(Wdt { regs }));
        {
            let dev = dev.clone();
            thread::Builder::new().name("Wdt").spawn(move || loop {
                dev.lock_read().service();
                syscall::Syscall::sleep_us(1_000_000);
            });
        }

        Ok(dev)
    }
}

#[initcall(priority = 0)]
fn wdt_register_driver() {
    super::register_driver(COMPATIBLE, Box::new(WdtDriver {})).unwrap();
}
