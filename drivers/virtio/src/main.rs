#![no_std]
#![no_main]

pub use driver_helper as _;

#[no_mangle]
fn driver_main() -> Result<(), ()> {
    Ok(())
}
