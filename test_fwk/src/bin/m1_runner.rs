use anyhow::anyhow;
use anyhow::Context;
use std::{error::Error, fs::File, io::Read, path::Path};
use structopt::StructOpt;
use toml::Value;
use xshell::{cmd, rm_rf};

#[derive(StructOpt)]
#[structopt(
    name = "m1_runner",
    about = "Run m1 emulator with the given ELF executable"
)]
struct Opts {
    fw_elf: String,

    #[structopt(long, short)]
    show_display: bool,

    #[structopt(long, short)]
    show_stdio: bool,
}

#[derive(Debug, Clone)]
struct Config {
    show_display: bool,
    show_stdio: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_stdio: false,
            show_display: false,
        }
    }
}

fn parse_config(config: &mut Config, manifest_path: &Path) -> anyhow::Result<()> {
    let cargo_toml: Value = {
        let mut content = String::new();
        File::open(manifest_path)
            .context("Failed to open Cargo.toml")?
            .read_to_string(&mut content)
            .context("Failed to read Cargo.toml")?;
        content
            .parse::<Value>()
            .context("Failed to parse Cargo.toml")?
    };

    let config_toml = match cargo_toml.get("m1_runner") {
        Some(config_toml) => config_toml
            .as_table()
            .ok_or_else(|| anyhow!("invalid m1_runner config found: {:?}", config_toml))?,
        None => {
            return Ok(());
        }
    };

    for (k, v) in config_toml {
        match k.as_str() {
            "show_display" => {
                let val = v
                    .as_bool()
                    .ok_or_else(|| anyhow!("show_display should be a boolean"))?;
                config.show_display = val;
            }
            "show_stdio" => {
                let val = v
                    .as_bool()
                    .ok_or_else(|| anyhow!("show_stdio should be a boolean"))?;
                config.show_stdio = val;
            }
            _ => {
                return Err(anyhow!("Unexpected key found in m1_config: {:?}", k));
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let opts = Opts::from_args();

    let mut config = Config::default();
    config.show_stdio = opts.show_stdio;
    config.show_display = opts.show_display;

    let manifest_path = std::env::var("CARGO_MANIFEST_DIR")
        .ok()
        .map(|dir| Path::new(&dir).join("Cargo.toml"))
        .expect("WARNING: `CARGO_MANIFEST_DIR` env variable not set");
    parse_config(&mut config, &manifest_path)?;

    cmd!("rust-objcopy")
        .arg("-O")
        .arg("binary")
        .arg(opts.fw_elf)
        .arg("_test_fw.macho")
        .run()?;
    let qemu_cmd = cmd!("qemu-system-aarch64 -machine apple-m1 -bios _test_fw.macho -semihosting");

    let mut additional_args = vec![];
    if !config.show_display {
        additional_args.push("--display");
        additional_args.push("none");
    };

    additional_args.push("-serial");

    if !config.show_stdio {
        additional_args.push("none");
    } else {
        additional_args.push("stdio");
    };

    qemu_cmd.args(additional_args.iter()).run()?;
    rm_rf("_test_fw.macho")?;
    Ok(())
}
