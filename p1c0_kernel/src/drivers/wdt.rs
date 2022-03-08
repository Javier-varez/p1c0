use tock_registers::{interfaces::Writeable, register_bitfields, registers::ReadWrite};

use crate::memory::address::Address;

// Defines bitfields for the WDT registers
register_bitfields![u32,
    /// Controls the state of the watchdog
    Control [
        ENABLE OFFSET(2) NUMBITS(1) [],
    ],
];

static mut WDT: Option<Wdt> = None;

unsafe impl Sync for Wdt {}

#[repr(C)]
struct WdtRegs {
    reserved1: [u32; 4],
    count: ReadWrite<u32>,
    alarm: ReadWrite<u32>,
    reserved2: u32,
    control: ReadWrite<u32, Control::Register>,
}

pub struct Wdt {
    regs: *mut WdtRegs,
}

// The watchdog seems to be running at 24 MHz by default.
// The programming sequence is:
//   * Reset the watchdog count to 0
//   * Write an alarm count. The system will be restarted when it is hit
//   * Enable the watchdog by writing the control register enable bit

impl Wdt {
    const FREQ_KHZ: u32 = 24_000;

    fn new() -> Self {
        let adt = crate::adt::get_adt().unwrap();
        let (pa, size) = adt.get_device_addr("/arm-io/wdt", 0).unwrap();

        let va = crate::memory::MemoryManager::instance()
            .map_io("Wdt regs", pa, size)
            .unwrap();

        let regs = va.as_mut_ptr() as *mut WdtRegs;

        const TIMEOUT_MS: u32 = 5_000;
        unsafe {
            (*regs).count.set(0);
            (*regs).alarm.set(Self::FREQ_KHZ * TIMEOUT_MS);
            (*regs).control.write(Control::ENABLE::SET);
        }

        Self { regs }
    }

    fn regs(&self) -> &'static WdtRegs {
        unsafe { &mut (*self.regs) }
    }

    fn service(&mut self) {
        let regs = self.regs();
        regs.count.set(0);
    }
}

unsafe fn instance() -> &'static mut Wdt {
    if WDT.is_none() {
        WDT.replace(Wdt::new());
    }
    WDT.as_mut().unwrap()
}

pub fn service() {
    unsafe { instance().service() };
}
