use xshell::cmd;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "xtask", about = "runs automated tasks with cargo")]
enum Options {
    /// Builds fw for p1c0
    Build {
        /// Builds the release version of the FW
        #[structopt(long)]
        release: bool,
    },
    /// Runs all tests
    Test,
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

fn main() -> Result<(), anyhow::Error> {
    let opts = Options::from_args();

    match opts {
        Options::Build { release } => build(release)?,
        Options::Test => run_tests()?,
        Options::Clippy => run_clippy()?,
    };

    Ok(())
}
