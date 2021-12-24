use xshell::cmd;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "xtask", about = "runs automated tasks with cargo")]
enum Options {
    /// Runs Qemu with the built FW.
    /// Make sure to use an M1 compatible qemu version like:
    /// <https://github.com/Javier-varez/qemu-apple-m1/suites/4750576312/artifacts/131568164>
    Run {
        /// Use the `release` FW.
        #[structopt(long)]
        release: bool,
    },
    /// Builds FW for p1c0. Generates a `.macho` file in the p1c0 folder.
    Build {
        /// Builds with the `release` profile.
        #[structopt(long)]
        release: bool,
    },
    /// Runs all tests.
    Test,
    /// Runs clippy on all sources.
    Clippy,
}

fn build(release: bool) -> Result<(), anyhow::Error> {
    let _dir = xshell::pushd("fw")?;
    let release = if release { Some("--release") } else { None };
    cmd!("cargo build").args(release).run()?;
    cmd!("cargo objcopy")
        .args(release)
        .arg("--")
        .arg("-O")
        .arg("binary")
        .arg("p1c0.macho")
        .run()?;
    Ok(())
}

fn run_tests() -> Result<(), anyhow::Error> {
    cmd!("cargo test").run()?;
    Ok(())
}

fn run_clippy() -> Result<(), anyhow::Error> {
    cmd!("cargo clippy").run()?;
    let _dir = xshell::pushd("fw")?;
    cmd!("cargo clippy").run()?;
    Ok(())
}

fn run_qemu(release: bool) -> Result<(), anyhow::Error> {
    build(release)?;
    cmd!("qemu-system-aarch64 -machine apple-m1 -bios fw/p1c0.macho -serial stdio --display none")
        .run()?;
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let opts = Options::from_args();

    match opts {
        Options::Run { release } => run_qemu(release)?,
        Options::Build { release } => build(release)?,
        Options::Test => run_tests()?,
        Options::Clippy => run_clippy()?,
    };

    Ok(())
}
