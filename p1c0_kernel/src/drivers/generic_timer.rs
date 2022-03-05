use cortex_a::{
    asm::barrier,
    registers::{CNTFRQ_EL0, CNTVCT_EL0, CNTV_CTL_EL0, CNTV_TVAL_EL0},
};
use tock_registers::interfaces::{Readable, Writeable};

use core::sync::atomic::{AtomicU32, Ordering};

pub struct GenericTimer {
    ticks_per_cycle: AtomicU32,
}

impl GenericTimer {
    const fn new() -> Self {
        Self {
            ticks_per_cycle: AtomicU32::new(0),
        }
    }
}

impl GenericTimer {
    pub fn resolution(&self) -> u64 {
        CNTFRQ_EL0.get()
    }

    pub fn initialize(&self, interval: core::time::Duration) {
        let ticks_per_cycle =
            ((CNTFRQ_EL0.get() * interval.as_nanos() as u64) / 1_000_000_000) as u32;
        crate::println!("Ticks per cycle {}", ticks_per_cycle);
        CNTV_TVAL_EL0.set(ticks_per_cycle as u64);
        CNTV_CTL_EL0.write(CNTV_CTL_EL0::IMASK::CLEAR + CNTV_CTL_EL0::ENABLE::SET);

        self.ticks_per_cycle
            .store(ticks_per_cycle, Ordering::Relaxed)
    }

    pub fn ticks(&self) -> u64 {
        // Ensures that we don't get an out of order value by adding an instruction barrier
        // (flushing the instruction pipeline)
        unsafe { barrier::isb(barrier::SY) };
        CNTVCT_EL0.get()
    }

    /// Delays execution for the given duration. Currently this is a blocking routine that does not
    /// sleep, just simply spins
    pub fn delay(&self, time: core::time::Duration) {
        const S_TO_NS: u128 = 1_000_000_000;
        let ticks = ((self.resolution() as u128 * time.as_nanos()) / S_TO_NS) as u64;
        let start = self.ticks();

        while self.ticks() < (start + ticks) {}
    }

    pub fn handle_irq(&self) {
        CNTV_TVAL_EL0.set(self.ticks_per_cycle.load(Ordering::Relaxed) as u64);
        CNTV_CTL_EL0.write(CNTV_CTL_EL0::IMASK::CLEAR + CNTV_CTL_EL0::ENABLE::SET);
    }

    pub fn is_irq_active(&self) -> bool {
        CNTV_CTL_EL0.matches_all(
            CNTV_CTL_EL0::IMASK::CLEAR + CNTV_CTL_EL0::ENABLE::SET + CNTV_CTL_EL0::ISTATUS::SET,
        )
    }
}

static GENERIC_TIMER: GenericTimer = GenericTimer::new();

pub fn get_timer() -> &'static GenericTimer {
    &GENERIC_TIMER
}
