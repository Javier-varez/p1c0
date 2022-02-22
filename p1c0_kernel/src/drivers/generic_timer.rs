use cortex_a::{
    asm::barrier,
    registers::{CNTFRQ_EL0, CNTVCT_EL0},
};
use tock_registers::interfaces::Readable;

pub struct GenericTimer {}

impl GenericTimer {
    const fn new() -> Self {
        Self {}
    }
}

impl GenericTimer {
    pub fn resolution(&self) -> u64 {
        CNTFRQ_EL0.get()
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
}

static GENERIC_TIMER: GenericTimer = GenericTimer::new();

pub fn get_timer() -> &'static GenericTimer {
    &GENERIC_TIMER
}
