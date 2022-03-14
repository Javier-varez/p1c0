use p1c0_kernel::{
    memory::{address::VirtualAddress, Permissions},
    process,
};

const TEXT: &[u8] = include_bytes!("../../userspace_test/build/userspace_test_text.bin");
const RODATA: &[u8] = include_bytes!("../../userspace_test/build/userspace_test_rodata.bin");
const DATA: &[u8] = include_bytes!("../../userspace_test/build/userspace_test_data.bin");

pub fn create_process() {
    process::Builder::new()
        .map_section(
            ".text",
            VirtualAddress::try_from_ptr(0x0000000001000000 as *const _).unwrap(),
            TEXT,
            Permissions::RWX,
        )
        .map_section(
            ".rodata",
            VirtualAddress::try_from_ptr(0x0000000001004000 as *const _).unwrap(),
            RODATA,
            Permissions::RWX,
        )
        .map_section(
            ".data",
            VirtualAddress::try_from_ptr(0x0000000001008000 as *const _).unwrap(),
            DATA,
            Permissions::RWX,
        )
        .start(VirtualAddress::try_from_ptr(0x0000000001000000 as *const _).unwrap())
        .unwrap();
}
