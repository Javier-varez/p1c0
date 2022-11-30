use super::interfaces::{self, TimerResolution};

use core::sync::atomic::{AtomicU32, Ordering};

use aarch64_cpu::{
    asm::barrier,
    registers::{CNTFRQ_EL0, CNTVCT_EL0, CNTV_CTL_EL0, CNTV_TVAL_EL0},
};
use tock_registers::interfaces::{Readable, Writeable};

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

impl interfaces::timer::Timer for GenericTimer {
    fn initialize(&self, interval: core::time::Duration) {
        let ticks_per_cycle =
            ((CNTFRQ_EL0.get() * interval.as_nanos() as u64) / 1_000_000_000) as u32;
        CNTV_TVAL_EL0.set(ticks_per_cycle as u64);
        CNTV_CTL_EL0.write(CNTV_CTL_EL0::IMASK::CLEAR + CNTV_CTL_EL0::ENABLE::SET);

        self.ticks_per_cycle
            .store(ticks_per_cycle, Ordering::Relaxed)
    }

    fn resolution(&self) -> TimerResolution {
        TimerResolution::from_hz(CNTFRQ_EL0.get())
    }

    fn ticks(&self) -> interfaces::Ticks {
        // Ensures that we don't get an out of order value by adding an instruction barrier
        // (flushing the instruction pipeline)
        barrier::isb(barrier::SY);
        interfaces::Ticks::new(CNTVCT_EL0.get())
    }

    fn handle_irq(&self) {
        CNTV_TVAL_EL0.set(self.ticks_per_cycle.load(Ordering::Relaxed) as u64);
        CNTV_CTL_EL0.write(CNTV_CTL_EL0::IMASK::CLEAR + CNTV_CTL_EL0::ENABLE::SET);
    }

    fn is_irq_active(&self) -> bool {
        CNTV_CTL_EL0.matches_all(
            CNTV_CTL_EL0::IMASK::CLEAR + CNTV_CTL_EL0::ENABLE::SET + CNTV_CTL_EL0::ISTATUS::SET,
        )
    }
}

// TODO(javier-varez): As with everything else, this should be moved towards a more
// generic interface where we instantiate everything from the ADT.
static GENERIC_TIMER: GenericTimer = GenericTimer::new();

pub fn get_timer() -> &'static GenericTimer {
    &GENERIC_TIMER
}
