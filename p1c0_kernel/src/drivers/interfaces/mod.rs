pub mod timer;

use core::time::Duration;

/// The current number of ticks the timer has made since boot
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub struct Ticks(u64);

impl Ticks {
    /// This method is only recommended for implementors of the Timer trait. If you REALLY insist in
    /// using this method, at least make sure that the ticks really mean something to the driver
    pub(super) fn new(raw_ticks: u64) -> Ticks {
        Self(raw_ticks)
    }
}

/// Resolution for a timer.
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub struct TimerResolution(u64);

impl TimerResolution {
    const S_IN_NS: u128 = 1_000_000_000;

    pub(super) fn from_hz(hz: u64) -> TimerResolution {
        TimerResolution(hz)
    }

    pub fn into_hz(self) -> u64 {
        self.0
    }
    pub fn into_duration(self) -> Duration {
        Duration::from_nanos(Self::S_IN_NS as u64 / self.0)
    }

    pub fn ticks_to_duration(&self, ticks: Ticks) -> Duration {
        Duration::from_nanos(((ticks.0 as u128 * Self::S_IN_NS) / self.0 as u128) as u64)
    }

    pub fn duration_to_ticks(&self, duration: Duration) -> Ticks {
        Ticks(((duration.as_nanos() as u128 * self.0 as u128) / Self::S_IN_NS as u128) as u64)
    }
}
