use crate::{
    memory::address::{Address, Validator, VirtualAddress},
    prelude::*,
};

use core::fmt::Formatter;

#[repr(C)]
struct Frame {
    next: *const Frame,
    lr: *const u8,
}

pub trait Symbolicator {
    fn symbolicate(&self, addr: VirtualAddress) -> Option<(String, usize)>;
}

#[derive(Clone)]
pub struct StackFrameIter<V: Validator, S: Symbolicator> {
    frame_ptr: VirtualAddress,
    validator: V,
    symbolicator: Option<S>,
}

impl<V: Validator, S: Symbolicator> Iterator for StackFrameIter<V, S> {
    type Item = (VirtualAddress, Option<(String, usize)>);

    fn next(&mut self) -> Option<Self::Item> {
        if !self.validator.is_valid(self.frame_ptr) {
            return None;
        }

        let frame_ptr = self.frame_ptr.as_ptr() as *const Frame;

        // # Safety: This should be safe because it is within the validated range
        let item = VirtualAddress::new_unaligned(unsafe { (*frame_ptr).lr });

        self.frame_ptr = VirtualAddress::new_unaligned(unsafe { (*frame_ptr).next } as *const _);

        // We hit the end on nullptr
        if item.as_ptr().is_null() {
            return None;
        }

        let symbol = if let Some(symbolicator) = &self.symbolicator {
            symbolicator.symbolicate(item)
        } else {
            None
        };

        Some((item, symbol))
    }
}

impl<V: Validator + Clone, S: Symbolicator + Clone> core::fmt::Display for StackFrameIter<V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let iter_clone = (*self).clone();
        writeln!(f, "Stack trace:")?;
        for (level, (frame, symbol)) in iter_clone.enumerate() {
            if let Some((symbol_name, symbol_offset)) = symbol {
                writeln!(
                    f,
                    "\t[{}] = {} - {} (+0x{:x})",
                    level, frame, symbol_name, symbol_offset
                )?;
            } else {
                writeln!(f, "\t[{}] = {}", level, frame)?;
            }
        }
        Ok(())
    }
}

pub fn stack_frame_iter<V, S>(
    frame_ptr: VirtualAddress,
    validator: V,
    symbolicator: Option<S>,
) -> StackFrameIter<V, S>
where
    V: Validator + Clone,
    S: Symbolicator + Clone,
{
    StackFrameIter {
        frame_ptr,
        validator,
        symbolicator,
    }
}

pub mod ksyms {
    use super::Symbolicator;
    use alloc::borrow::ToOwned;

    use crate::{
        init,
        memory::address::{Address, VirtualAddress},
        sync::spinlock::RwSpinLock,
    };

    use alloc::string::String;

    static KSYMS: RwSpinLock<Option<KSyms>> = RwSpinLock::new(None);

    mod header {
        pub const MAGIC: [u8; 4] = *b"Smbl";

        pub const MAGIC_OFFSET: usize = 0x00;
        pub const FILESIZE_OFFSET: usize = 0x04;
        pub const NUM_SYMBOLS_OFFSET: usize = 0x08;
        pub const SYMBOL_TABLE_OFFSET_OFFSET: usize = 0x0C;
        pub const STRING_TABLE_OFFSET_OFFSET: usize = 0x10;

        pub const SIZE: usize = 0x14;
    }

    mod entry {
        pub const ENTRY_NAME_OFFSET_OFFSET: usize = 0x00;
        pub const ENTRY_NAME_LENGTH_OFFSET: usize = 0x04;
        pub const ENTRY_ADDRESS_OFFSET: usize = 0x08;
        pub const ENTRY_SIZE_OFFSET: usize = 0x10;

        pub const SIZE: usize = 24;
    }

    macro_rules! read_u32 {
        ($buffer: expr, $offset: expr) => {
            $buffer[$offset] as u32
                | ($buffer[$offset + 1] as u32) << 8
                | ($buffer[$offset + 2] as u32) << 16
                | ($buffer[$offset + 3] as u32) << 24
        };
    }

    macro_rules! read_u64 {
        ($buffer: expr, $offset: expr) => {
            $buffer[$offset] as u64
                | ($buffer[$offset + 1] as u64) << 8
                | ($buffer[$offset + 2] as u64) << 16
                | ($buffer[$offset + 3] as u64) << 24
                | ($buffer[$offset + 4] as u64) << 32
                | ($buffer[$offset + 5] as u64) << 40
                | ($buffer[$offset + 6] as u64) << 48
                | ($buffer[$offset + 7] as u64) << 56
        };
    }

    #[derive(Clone)]
    pub struct KSyms {
        base_address: VirtualAddress,
        symbol_table_data: &'static [u8],
        string_table_data: &'static [u8],
    }

    pub(crate) fn parse(data: &'static [u8]) -> Result<usize, ()> {
        if data[header::MAGIC_OFFSET..header::MAGIC_OFFSET + core::mem::size_of_val(&header::MAGIC)]
            != header::MAGIC
        {
            return Err(());
        }

        let header = &data[..header::SIZE];

        let filesize = read_u32!(header, header::FILESIZE_OFFSET) as usize;
        let data = &data[..filesize];

        let symbol_table_offset = read_u32!(header, header::SYMBOL_TABLE_OFFSET_OFFSET) as usize;
        let num_symbols = read_u32!(header, header::NUM_SYMBOLS_OFFSET) as usize;
        let string_table_offset = read_u32!(header, header::STRING_TABLE_OFFSET_OFFSET) as usize;

        let symbol_table_data =
            &data[symbol_table_offset..symbol_table_offset + num_symbols * entry::SIZE];

        let string_table_data = &data[string_table_offset..];

        let ksyms = KSyms {
            base_address: init::get_base(),
            symbol_table_data,
            string_table_data,
        };

        let prev_syms = KSYMS.lock_write().replace(ksyms);
        assert!(prev_syms.is_none(), "KSyms are duplicated in payload!");

        Ok(filesize)
    }

    enum EntryMatch {
        Previous,
        Match(Option<(String, usize)>),
        Next,
    }

    impl KSyms {
        fn get_name(&self, name_offset: usize, name_length: usize) -> Option<&str> {
            let data = &self.string_table_data[name_offset..name_offset + name_length];
            core::str::from_utf8(data).ok()
        }

        fn matches_entry(&self, entry_data: &[u8], addr: usize) -> EntryMatch {
            let symbol_start = read_u64!(entry_data, entry::ENTRY_ADDRESS_OFFSET) as usize;
            let symbol_size = read_u64!(entry_data, entry::ENTRY_SIZE_OFFSET) as usize;

            if addr < symbol_start {
                EntryMatch::Previous
            } else if addr >= (symbol_start + symbol_size) {
                EntryMatch::Next
            } else {
                let name_offset = read_u32!(entry_data, entry::ENTRY_NAME_OFFSET_OFFSET) as usize;
                let name_length = read_u32!(entry_data, entry::ENTRY_NAME_LENGTH_OFFSET) as usize;

                EntryMatch::Match(
                    self.get_name(name_offset, name_length)
                        .map(|name| (name.to_owned(), addr - symbol_start)),
                )
            }
        }
    }

    impl Symbolicator for KSyms {
        fn symbolicate(&self, addr: VirtualAddress) -> Option<(String, usize)> {
            let addr = addr.remove_base(self.base_address).as_usize();

            let mut symbol_table_data = self.symbol_table_data;
            loop {
                let num_entries = symbol_table_data.len() / entry::SIZE;

                // For small N just do a linear search
                if num_entries < 5 {
                    // Just do linear search for small num of entries
                    for i in 0..num_entries {
                        let entry_data = &symbol_table_data[i * entry::SIZE..(i + 1) * entry::SIZE];

                        if let EntryMatch::Match(result) = self.matches_entry(entry_data, addr) {
                            return result;
                        }
                    }
                    return None;
                }

                let middle_index = num_entries / 2;

                let entry_data = &symbol_table_data
                    [middle_index * entry::SIZE..(middle_index + 1) * entry::SIZE];

                match self.matches_entry(entry_data, addr) {
                    EntryMatch::Previous => {
                        symbol_table_data = &symbol_table_data[..middle_index * entry::SIZE]
                    }
                    EntryMatch::Next => {
                        symbol_table_data = &symbol_table_data[(middle_index + 1) * entry::SIZE..]
                    }
                    EntryMatch::Match(result) => return result,
                };
            }
        }
    }

    pub fn symbolicator() -> Option<KSyms> {
        KSYMS.lock_read().as_ref().cloned()
    }
}
