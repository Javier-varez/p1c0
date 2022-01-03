use std::error::Error;
use structopt::StructOpt;
use xshell::{cmd, rm_rf};

#[derive(StructOpt)]
#[structopt(
    name = "m1_runner",
    about = "Run m1 emulator with the given ELF executable"
)]
struct Opts {
    fw_elf: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opts::from_args();

    cmd!("rust-objcopy")
        .arg("-O")
        .arg("binary")
        .arg(opts.fw_elf)
        .arg("_test_fw.macho")
        .run()?;
    cmd!("qemu-system-aarch64 -machine apple-m1 -bios _test_fw.macho -serial none --display none -semihosting")
        .run()?;
    rm_rf("_test_fw.macho")?;
    Ok(())
}
