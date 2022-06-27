#![no_std]

mod relocation;
pub mod syscall;

use relocation::RelaEntry;

#[panic_handler]
fn panic_handler(_panic_info: &core::panic::PanicInfo) -> ! {
    syscall::exit(1)
}

extern "Rust" {
    fn driver_main() -> Result<(), ()>;
}

extern "C" {
    static _rela_start: u8;
    static _rela_end: u8;
}

#[no_mangle]
unsafe fn _start(_argc: usize, _argv: usize, _envp: usize, base_addr: usize) {
    // This is the entrypoint for rust
    let rela_start = &_rela_start as *const _ as *const RelaEntry;
    let rela_end = &_rela_end as *const _ as *const RelaEntry;
    let rela_size = rela_end.offset_from(rela_start) as usize * core::mem::size_of::<RelaEntry>();

    relocation::apply_rela(base_addr, rela_start, rela_size);

    syscall::print_str("hello world!!");
    driver_main().unwrap();

    syscall::print_str("Process done");
    syscall::exit(0);
}
