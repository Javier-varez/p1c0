use super::{Ticks, TimerResolution};

pub trait Timer {
    /// Initializes the timer to run at a fixed jiffy interval. This is not related to the timer
    /// resolution, which can and should be much higher than the interval
    fn initialize(&self, interval: core::time::Duration);

    /// Returns the resolution of the timer frequency in Mhz
    fn resolution(&self) -> TimerResolution;
    fn ticks(&self) -> Ticks;
    fn handle_irq(&self);
    fn is_irq_active(&self) -> bool;

    /// Delays execution for the given duration. Currently this is a blocking routine that does not
    /// sleep, just simply spins
    fn delay(&self, time: core::time::Duration) {
        const S_TO_NS: u128 = 1_000_000_000;
        let ticks = ((self.resolution().into_hz() as u128 * time.as_nanos()) / S_TO_NS) as u64;
        let start = self.ticks().0;

        while self.ticks().0 < (start + ticks) {}
    }
}
