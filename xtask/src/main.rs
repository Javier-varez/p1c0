use std::env::var;
use std::io::Read;
use std::process::exit;
use xshell::{cmd, pushenv, Pushenv};

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

        /// Targets the emulator and adds semihosting support
        #[structopt(long)]
        emulator: bool,

        /// Builds a binary file instead of a macho file. Can be used from macOS 12.2 onwards
        #[structopt(long)]
        binary: bool,
    },
    /// Runs all tests.
    Test,
    /// Runs clippy on all sources.
    Clippy,
    /// Installs requirements for the project
    /// These are m1_runner
    InstallRequirements,
}

fn build_rootfs() -> Result<(), anyhow::Error> {
    // Before building anything let's make sure to build the userspace tests
    cmd!("make -C userspace_test")
        .env("CROSS_COMPILE", "aarch64-none-elf-")
        .run()?;

    Ok(())
}

struct Env(Vec<Pushenv>);

fn configure_environment() -> Result<Env, anyhow::Error> {
    let mut env_settings = Env(vec![]);

    // load path from .env file
    let mut env_file = match std::fs::File::open(".env") {
        Ok(file) => file,
        Err(_) => {
            println!("No .env file found");
            return Ok(env_settings);
        }
    };
    let mut env_str = String::new();
    env_file.read_to_string(&mut env_str)?;

    for line in env_str.split('\n') {
        let line = line.trim();
        if line.len() == 0 {
            // Ignore empty lines
            continue;
        }

        let mut split = line.split_whitespace();
        let var_name = match split.next() {
            Some(val) => val,
            None => {
                println!(".env file is malformed. Missing variable name");
                exit(1);
            }
        };

        let operation = match split.next() {
            Some(val) => val,
            None => {
                println!(".env file is malformed. Missing operation");
                exit(1);
            }
        };

        let argument = match split.next() {
            Some(val) => val,
            None => {
                println!(".env file is malformed. Missing argument");
                exit(1);
            }
        };

        match operation {
            "=" => {
                env_settings.0.push(pushenv(var_name, argument));
            }
            "+=" => {
                // Read var first
                let old_value = std::env::var(var_name)?;
                let mut new_value = argument.to_string();
                new_value.push(':');
                new_value.push_str(&old_value);
                env_settings.0.push(pushenv(var_name, &new_value));
            }
            op => {
                println!("Unknown env operation `{}`", op);
                exit(1);
            }
        }
    }

    Ok(env_settings)
}

fn build(release: bool, emulator: bool, binary: bool) -> Result<(), anyhow::Error> {
    build_rootfs()?;

    let _dir = xshell::pushd("fw")?;
    let release = if release { Some("--release") } else { None };

    let mut build_features = vec![];
    if emulator {
        build_features.push("emulator");
    }
    if binary {
        build_features.push("binary");
    }

    let features = if build_features.is_empty() {
        None
    } else {
        let mut feature_string = "--features=".to_string();
        let num_features = build_features.len();
        for (index, feature) in build_features.iter().enumerate() {
            feature_string.push_str(feature);
            if index != (num_features - 1) {
                feature_string.push(',');
            }
        }
        Some(feature_string)
    };

    let output_name = if binary { "p1c0.bin" } else { "p1c0.macho" };

    cmd!("cargo build")
        .args(release)
        .args(features.clone())
        .run()?;
    cmd!("cargo objcopy")
        .args(release)
        .args(features)
        .arg("--")
        .arg("-O")
        .arg("binary")
        .arg(output_name)
        .run()?;
    Ok(())
}

fn check_prerequisites() -> Result<(), anyhow::Error> {
    if cmd!("m1_runner -V").run().is_err() {
        install_requirements()?;
    }
    Ok(())
}

fn run_tests() -> Result<(), anyhow::Error> {
    build_rootfs()?;

    // Run host tests
    cmd!("cargo test").run()?;

    // run FW tests
    check_prerequisites()?;
    let _dir = xshell::pushd("fw")?;
    cmd!("cargo test").run()?;
    Ok(())
}

fn run_clippy() -> Result<(), anyhow::Error> {
    build_rootfs()?;
    cmd!("cargo clippy").run()?;
    let _dir = xshell::pushd("fw")?;
    cmd!("cargo clippy").run()?;
    Ok(())
}

fn run_qemu(release: bool) -> Result<(), anyhow::Error> {
    build(release, true, false)?;
    cmd!("qemu-system-aarch64 -machine apple-m1 -bios fw/p1c0.macho -serial stdio --display none -semihosting")
        .run()?;
    Ok(())
}

fn install_requirements() -> Result<(), anyhow::Error> {
    println!("Installing requirements");
    println!("\tm1_runner:");
    cmd!("cargo install --path test_fwk --features m1_runner").run()?;
    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let opts = Options::from_args();

    configure_environment();

    match opts {
        Options::Run { release } => run_qemu(release)?,
        Options::Build {
            release,
            emulator,
            binary,
        } => build(release, emulator, binary)?,
        Options::Test => run_tests()?,
        Options::Clippy => run_clippy()?,
        Options::InstallRequirements => install_requirements()?,
    };

    Ok(())
}
