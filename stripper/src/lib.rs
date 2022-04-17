use object::{
    elf, endian, read::elf::ElfFile, Object, ObjectSymbol, ObjectSymbolTable, SymbolKind,
};
use rustc_demangle::demangle;

struct Symbol {
    address: u64,
    size: u64,
    name_offset: u32,
    name_length: u32,
}

pub fn symbols_from_elf_file(
    elf: &ElfFile<elf::FileHeader64<endian::LittleEndian>>,
    symbol_file: &mut impl std::io::Write,
) -> anyhow::Result<()> {
    let mut symbols = vec![];
    let mut string_table: Vec<u8> = vec![];

    if let Some(symbol_table) = elf.symbol_table() {
        symbol_table
            .symbols()
            .filter(|symbol| symbol.kind() == SymbolKind::Text)
            .for_each(|symbol| {
                if let Ok(name) = symbol.name() {
                    let name = demangle(name).to_string();

                    let name_offset = string_table.len() as u32;
                    let name_length = name.bytes().len() as u32;

                    name.bytes().for_each(|byte| string_table.push(byte));

                    symbols.push(Symbol {
                        address: symbol.address(),
                        size: symbol.size(),
                        name_offset,
                        name_length,
                    });
                } else {
                    panic!("Symbol has invalid name!");
                }
            });
        // Sort symbols by address
        symbols.sort_by(|a, b| a.address.cmp(&b.address));
    }

    const MAGIC_BYTES: [u8; 4] = *b"Smbl";
    const SYMBOL_TABLE_OFFSET: u32 = 0x14;
    const SYMBOL_ENTRY_SIZE: u32 = 0x18;

    let num_symbols = symbols.len() as u32;
    let string_table_offset = SYMBOL_TABLE_OFFSET + num_symbols * SYMBOL_ENTRY_SIZE;
    let filesize = string_table_offset + string_table.len() as u32;

    symbol_file.write_all(&MAGIC_BYTES)?;
    symbol_file.write_all(&u32::to_le_bytes(filesize))?;
    symbol_file.write_all(&u32::to_le_bytes(symbols.len() as u32))?;
    symbol_file.write_all(&u32::to_le_bytes(SYMBOL_TABLE_OFFSET))?;
    symbol_file.write_all(&u32::to_le_bytes(string_table_offset))?;

    for symbol in symbols {
        symbol_file.write_all(&u32::to_le_bytes(symbol.name_offset))?;
        symbol_file.write_all(&u32::to_le_bytes(symbol.name_length))?;
        symbol_file.write_all(&u64::to_le_bytes(symbol.address))?;
        symbol_file.write_all(&u64::to_le_bytes(symbol.size))?;
    }

    symbol_file.write_all(&string_table[..])?;

    Ok(())
}
