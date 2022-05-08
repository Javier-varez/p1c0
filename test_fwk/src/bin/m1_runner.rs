use anyhow::anyhow;
use anyhow::Context;
use object::read::elf::ElfFile;
use std::{error::Error, fs::File, io::Read, path::Path};
use std::{fs, io::Write};
use structopt::StructOpt;
use toml::Value;
use xshell::{cmd, rm_rf};

#[derive(StructOpt)]
#[structopt(
    name = "m1_runner",
    about = "Run m1 emulator with the given ELF executable"
)]
struct Opts {
    fw_elf: std::path::PathBuf,

    #[structopt(long, short)]
    show_display: bool,

    #[structopt(long, short)]
    show_stdio: bool,

    #[structopt(long, short)]
    debug: bool,
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

fn build_macho_executable_with_payload(
    elf: &std::path::Path,
    macho_exec: &std::path::Path,
) -> anyhow::Result<()> {
    let objcopy_output = cmd!("rust-objcopy")
        .arg("-O")
        .arg("binary")
        .arg(&elf)
        .arg("-")
        .output()?;

    let mut macho_exec = std::fs::File::create(&macho_exec)?;
    macho_exec.write_all(&objcopy_output.stdout[..])?;

    // Now symbolicate and append that as well
    let elf_file = fs::read(&elf)?;
    let elf_file = ElfFile::parse(&elf_file[..]).map_err(|err| {
        anyhow::Error::msg(format!(
            "Input file is not in ELF64 Little endian format: {}",
            err
        ))
    })?;

    // Append symbols to file
    stripper::symbols_from_elf_file(&elf_file, &mut macho_exec)?;

    // Flush the mach-o file
    macho_exec.flush()?;
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

    let temp_file_name = opts
        .fw_elf
        .parent()
        .map(|parent| {
            let mut parent = parent.to_owned();
            parent.push("_tmp_fw.macho");
            parent
        })
        .ok_or(anyhow::Error::msg("fw_elf path does not have a parent"))?;

    // This makes sure the file is deleted before exiting
    {
        let ctrlc_temp_filename = temp_file_name.clone();
        ctrlc::set_handler(move || {
            rm_rf(&ctrlc_temp_filename).unwrap();
        })?;
    }

    build_macho_executable_with_payload(&opts.fw_elf, &temp_file_name)?;

    let qemu_cmd =
        cmd!("qemu-system-aarch64 -machine apple-m1 -bios {temp_file_name} -semihosting -device virtio-keyboard-device");

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

    if opts.debug {
        additional_args.push("-s");
        additional_args.push("-S");
    }

    qemu_cmd.args(additional_args.iter()).run()?;

    rm_rf(temp_file_name)?;
    Ok(())
}
