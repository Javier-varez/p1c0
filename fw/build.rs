use cc::Build;
use std::{env, error::Error, fs::File, io::Write, path::PathBuf};
use xshell::cmd;

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    println!("cargo:rustc-link-search={}", out_dir.display());

    #[cfg(feature = "binary")]
    File::create(out_dir.join("custom_p1c0.ld"))?.write_all(include_bytes!("p1c0_bin.ld"))?;

    #[cfg(not(feature = "binary"))]
    File::create(out_dir.join("custom_p1c0.ld"))?.write_all(include_bytes!("p1c0.ld"))?;

    Build::new()
        .file("startup.S")
        .target("aarch64-unknown-none-softfloat")
        .compiler("aarch64-none-elf-gcc")
        .compile("entry");

    println!("cargo:rerun-if-changed=startup.S");
    println!("cargo:rerun-if-changed=../userspace_test");

    Ok(())
}
