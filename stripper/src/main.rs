use std::fs;

use object::read::elf::ElfFile;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
struct Options {
    elf_file: std::path::PathBuf,
    symbol_file: std::path::PathBuf,
}

fn main() -> anyhow::Result<()> {
    let options = Options::from_args();

    let elf_file = fs::read(options.elf_file)?;
    let elf_file = ElfFile::parse(&elf_file[..])?;
    let mut symbol_file = fs::File::create(options.symbol_file)?;

    stripper::symbols_from_elf_file(&elf_file, &mut symbol_file)
}
