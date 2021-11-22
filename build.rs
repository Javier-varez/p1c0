use cc::Build;
use std::{env, error::Error, fs::File, io::Write, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());

    println!("cargo:rustc-link-search={}", out_dir.display());

    File::create(out_dir.join("p1c0.ld"))?.write_all(include_bytes!("p1c0.ld"))?;

    Build::new()
        .file("startup.S")
        .target("aarch64-unknown-none-softfloat")
        .compiler("aarch64-none-elf-gcc")
        .compile("entry");

    println!("cargo:rerun-if-changed=startup.S");

    Ok(())
}
