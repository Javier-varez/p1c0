use cortex_a::{
    asm::barrier,
    registers::{CNTFRQ_EL0, CNTPCT_EL0},
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
        CNTPCT_EL0.get()
    }
}

static GENERIC_TIMER: GenericTimer = GenericTimer::new();

pub fn get_timer() -> &'static GenericTimer {
    &GENERIC_TIMER
}
