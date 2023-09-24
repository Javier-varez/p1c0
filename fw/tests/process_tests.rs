#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]

use p1c0 as _; // needed to link libentry (and _start)

use p1c0_kernel::{
    filesystem::{OpenMode, VirtualFileSystem},
    prelude::*,
    process,
    syscall::Syscall,
    thread,
};

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    thread::Builder::new().name("Test").spawn(|| {
        test_main();
    });

    thread::initialize();
}

#[test_case]
fn test_fail_process() {
    let mut file = VirtualFileSystem::open("/bin/false", OpenMode::Read).unwrap();
    let mut elf_data = vec![];
    elf_data.resize(file.size, 0);

    VirtualFileSystem::read(&mut file, &mut elf_data[..]).unwrap();
    VirtualFileSystem::close(file);

    let builder = process::Builder::new_from_elf_data("/bin/false", elf_data, 0).unwrap();
    let pid = builder.start().unwrap();
    assert_eq!(Syscall::wait_pid(pid.get_raw()), 1);
}

#[test_case]
fn test_pass_process() {
    let mut file = VirtualFileSystem::open("/bin/true", OpenMode::Read).unwrap();
    let mut elf_data = vec![];
    elf_data.resize(file.size, 0);

    VirtualFileSystem::read(&mut file, &mut elf_data[..]).unwrap();
    VirtualFileSystem::close(file);

    let builder = process::Builder::new_from_elf_data("/bin/true", elf_data, 0).unwrap();
    let pid = builder.start().unwrap();
    assert_eq!(Syscall::wait_pid(pid.get_raw()), 0);
}

#[test_case]
fn test_process_crash() {
    let mut file = VirtualFileSystem::open("/bin/crash", OpenMode::Read).unwrap();
    let mut elf_data = vec![];
    elf_data.resize(file.size, 0);

    VirtualFileSystem::read(&mut file, &mut elf_data[..]).unwrap();
    VirtualFileSystem::close(file);

    let builder = process::Builder::new_from_elf_data("/bin/crash", elf_data, 0).unwrap();
    let pid = builder.start().unwrap();
    assert_eq!(Syscall::wait_pid(pid.get_raw()), 0xdeadc0de);
}
