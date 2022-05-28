pub mod aic;
pub mod display;
pub mod generic_timer;
pub mod gpio;
pub mod hid;
pub mod interfaces;
pub mod spi;
pub mod uart;
pub mod virtio;
pub mod wdt;

use crate::{adt::AdtNode, prelude::*, sync::spinlock::RwSpinLock};

#[derive(Debug)]
pub enum Error {
    DriverAlreadyRegistered(String),
    NoCompatibleInDevice,
    NoDriverForDevice,
    DeviceSpecificError(Box<dyn error::Error>),
}

pub type Result<T> = core::result::Result<T, Error>;

type DeviceRef = Arc<RwSpinLock<dyn Device>>;

trait Driver {
    fn probe(&self, dev_path: &[AdtNode]) -> Result<DeviceRef>;
}

trait Device {
    // What behaviors should devices expose?
}

// This just keeps devices alive for now, but should also allow to query devices from other devs.
#[allow(dead_code)]
static DEVICES: RwSpinLock<FlatMap<String, DeviceRef>> =
    RwSpinLock::new(FlatMap::new_no_capacity());

static DRIVERS: RwSpinLock<FlatMap<String, Box<dyn Driver>>> =
    RwSpinLock::new(FlatMap::new_no_capacity());

// Registration of drivers is only allowed from the driver module and submodules
fn register_driver(compatible: &str, driver: Box<dyn Driver>) -> Result<()> {
    let mut drivers = DRIVERS.lock_write();
    drivers
        .insert_with_strategy(
            compatible.to_string(),
            driver,
            flat_map::InsertStrategy::NoReplaceResize,
        )
        .map_err(|_| Error::DriverAlreadyRegistered(compatible.to_string()))?;
    Ok(())
}

pub fn probe_device(dev_path: &[AdtNode]) -> Result<()> {
    // Find a compatible driver and try to probe the device with it.
    // If that doesn't work we might need to cry and raise an error
    let dev = dev_path
        .last()
        .expect("There's no device to probe!")
        .clone();
    let compatible_list = dev
        .get_compatible_list()
        .ok_or(Error::NoCompatibleInDevice)?;

    for compatible_str in compatible_list {
        let drivers = DRIVERS.lock_read();
        if let Some(driver) = drivers.lookup(compatible_str) {
            driver.probe(dev_path)?;
        }
    }

    Err(Error::NoDriverForDevice)
}
