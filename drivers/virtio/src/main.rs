#![no_std]
#![no_main]

#[allow(unused_imports)]
pub use driver_helper as _;

#[no_mangle]
fn driver_main() -> Result<(), ()> {
    Ok(())
}
