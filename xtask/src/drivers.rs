use xshell::cmd;

use std::path::PathBuf;

const DRIVERS_DIR: &str = "drivers";
const DRIVERS: [&str; 1] = ["virtio"];

pub fn build() -> Result<(), anyhow::Error> {
    let rootfs = crate::ROOTFS_DIR;
    for driver in DRIVERS {
        let driver_dir = PathBuf::from(DRIVERS_DIR).join(driver);
        cmd!("cargo install --path {driver_dir} --root {rootfs}").run()?;
    }
    Ok(())
}
