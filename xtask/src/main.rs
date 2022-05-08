use std::io::{Read, Write};
use std::process::exit;
use xshell::{cmd, mkdir_p, pushd, pushenv, rm_rf, Pushenv};

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
    /// Removes all target directories
    Clean,
    /// Collects coverage information from integration tests and creates an HTML report
    Coverage,
}

const FW_DIR: &str = "fw";
const USERSPACE_DIR: &str = "userspace";
const BUILD_DIR: &str = "build";
const ROOTFS_DIR: &str = "build/rootfs";
const ROOTFS_FILE: &str = "build/rootfs.cpio";

fn build_rootfs() -> Result<(), anyhow::Error> {
    mkdir_p(ROOTFS_DIR)?;

    // Build userspace binaries
    cmd!("cmake -S {USERSPACE_DIR} -B {BUILD_DIR}/{USERSPACE_DIR} -DCMAKE_TOOLCHAIN_FILE=toolchain/aarch64.cmake -DCMAKE_SYSTEM_NAME=Generic").run()?;
    cmd!("cmake --build {BUILD_DIR}/{USERSPACE_DIR}").run()?;
    cmd!("cmake --install {BUILD_DIR}/{USERSPACE_DIR} --prefix {ROOTFS_DIR}").run()?;

    let rootfs_cpio_data = {
        let _dir = pushd(ROOTFS_DIR);
        let output = cmd!("find . -depth -print ").output()?;
        if !output.status.success() {
            println!("Error finding rootfs data");
            exit(1);
        }

        let rootfs_files = output.stdout;
        let output = cmd!("cpio -o -H newc").stdin(&rootfs_files[..]).output()?;

        if !output.status.success() {
            println!("Error creating rootfs cpio archive");
            exit(1);
        }
        output.stdout
    };

    let mut file = std::fs::File::create(ROOTFS_FILE)?;
    file.write(&rootfs_cpio_data[..])?;

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

fn get_cargo_args(
    release: bool,
    emulator: bool,
    binary: bool,
) -> Result<(Option<String>, Option<String>), anyhow::Error> {
    let release = if release {
        Some("--release".to_string())
    } else {
        None
    };

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

    Ok((release, features))
}

fn run_build(release: bool, emulator: bool, binary: bool) -> Result<(), anyhow::Error> {
    build_rootfs()?;

    let _dir = xshell::pushd(FW_DIR)?;
    let (release, features) = get_cargo_args(release, emulator, binary)?;

    let output_name = if binary { "p1c0.bin" } else { "p1c0.macho" };
    cmd!("cargo build")
        .args(release.clone())
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
    let _dir = xshell::pushd(FW_DIR)?;
    cmd!("cargo test").run()?;
    Ok(())
}

fn run_clippy() -> Result<(), anyhow::Error> {
    build_rootfs()?;
    cmd!("cargo clippy").run()?;
    let _dir = xshell::pushd(FW_DIR)?;
    cmd!("cargo clippy").run()?;
    Ok(())
}

fn run_qemu(release: bool) -> Result<(), anyhow::Error> {
    build_rootfs()?;

    let _dir = xshell::pushd(FW_DIR)?;
    let (release, features) = get_cargo_args(release, true, false)?;

    cmd!("cargo run")
        .args(release)
        .args(features.clone())
        .arg("--")
        .arg("--show-stdio")
        .arg("--show-display")
        .run()?;
    Ok(())
}

fn run_clean() -> Result<(), anyhow::Error> {
    rm_rf(ROOTFS_DIR)?;
    cmd!("cargo clean").run()?;
    let _dir = pushd(FW_DIR);
    cmd!("cargo clean").run()?;
    Ok(())
}

fn run_coverage() -> Result<(), anyhow::Error> {
    build_rootfs()?;

    // run FW tests and trigger coverage
    let _dir = xshell::pushd(FW_DIR)?;

    let rustflags = vec![
        "-C",
        "link-arg=-Tcustom_p1c0.ld",
        "-C",
        "link-arg=-Map=p1c0.map",
        "-C",
        "relocation-model=pic",
        "-C",
        "link-arg=--no-apply-dynamic-relocs",
        "-C",
        "link-arg=-pie",
        "-C",
        "link-args=-znocopyreloc",
        "-C",
        "link-args=-znotext",
        "-C",
        "force-frame-pointers=yes",
        "-C",
        "instrument-coverage",
        "-Z",
        "no-profiler-runtime",
    ];
    let mut rustflags_str = String::new();
    for flag in rustflags {
        rustflags_str.push_str(flag);
        rustflags_str.push(' ');
    }

    let _env = xshell::pushenv("RUSTFLAGS", rustflags_str);
    cmd!("cargo test --features=coverage -- --profile").run()?;

    let profraws = cmd!("find . -iname *.profraw").output()?.stdout;
    let profraws = String::from_utf8(profraws)?;
    let profraws: Vec<&str> = profraws.split_whitespace().collect();

    rm_rf("coverage_report")?;

    cmd!("grcov -o coverage_report -t html -s .. -b target/aarch64-unknown-none-softfloat/debug/deps")
        .args(profraws)
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

    let _env = configure_environment()?;

    // Install any missing tools
    check_prerequisites()?;

    match opts {
        Options::Run { release } => run_qemu(release)?,
        Options::Build {
            release,
            emulator,
            binary,
        } => run_build(release, emulator, binary)?,
        Options::Test => run_tests()?,
        Options::Clippy => run_clippy()?,
        Options::InstallRequirements => install_requirements()?,
        Options::Clean => run_clean()?,
        Options::Coverage => run_coverage()?,
    };

    Ok(())
}
