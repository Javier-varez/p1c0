pub trait Watchdog: crate::drivers::Device {
    fn pet(&self);
}
