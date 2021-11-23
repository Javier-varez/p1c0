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
}

fn build(release: bool) -> Result<(), anyhow::Error> {
    let _dir = xshell::pushd("fw")?;
    let release = if release { Some("--release") } else { None };
    cmd!("cargo build").args(release).run()?;
    cmd!("cargo objcopy --release -- -O binary p1c0.macho").run()?;
    Ok(())
}

fn run_tests() -> Result<(), anyhow::Error> {
    cmd!("cargo test").run()?;
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let opts = Options::from_args();

    match opts {
        Options::Build { release } => build(release)?,
        Options::Test => run_tests()?,
    };

    Ok(())
}
