#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]

extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use m1::adt::get_adt;

#[allow(unused_imports)]
use p1c0::*;

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    test_main();
}

#[test_case]
fn test_adt_can_be_instantiated() {
    let _ = get_adt().unwrap();
}

#[test_case]
fn test_adt_get_root_node() {
    let adt = get_adt().unwrap();
    let _root_node = adt.find_node("/").unwrap();
}

#[test_case]
fn test_adt_get_invalid_node() {
    let adt = get_adt().unwrap();
    assert!(adt.find_node("").is_none());
}

#[test_case]
fn test_adt_get_uart_node() {
    let adt = get_adt().unwrap();

    assert!(adt.find_node("/arm-io/uart0").is_some());
}

#[test_case]
fn test_adt_get_valid_property() {
    let adt = get_adt().unwrap();
    let node = adt.find_node("/arm-io/uart0").unwrap();
    let prop = node.find_property("compatible").unwrap();

    let compatibles: Vec<_> = prop.str_list_value().collect();
    assert_eq!(compatibles, vec!["uart-1,samsung"]);
}
