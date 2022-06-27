use xshell::cmd;

const USERSPACE_DIR: &str = "userspace";
const BUILD_DIR: &str = "build";

pub fn build() -> Result<(), anyhow::Error> {
    let rootfs = crate::ROOTFS_DIR;
    // Build userspace binaries
    cmd!("cmake -S {USERSPACE_DIR} -B {BUILD_DIR}/{USERSPACE_DIR} -DCMAKE_TOOLCHAIN_FILE=toolchain/aarch64.cmake -DCMAKE_SYSTEM_NAME=Generic").run()?;
    cmd!("cmake --build {BUILD_DIR}/{USERSPACE_DIR}").run()?;
    cmd!("cmake --install {BUILD_DIR}/{USERSPACE_DIR} --prefix {rootfs}").run()?;
    Ok(())
}
