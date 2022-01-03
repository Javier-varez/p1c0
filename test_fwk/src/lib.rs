#![no_std]

use arm_semihosting::println;
use core::ops::Fn;

use core::panic::PanicInfo;

pub fn runner(tests: &[&dyn Testable]) {
    println!("Starting test execution");
    tests.iter().for_each(|test| test.run());
    println!("Done with test execution");
    arm_semihosting::exit(0);
}

pub fn runner_should_panic(tests: &[&dyn Testable]) {
    println!("Starting test execution");
    tests.iter().for_each(|test| test.run());
    println!("Done with test execution");
    arm_semihosting::exit(1);
}

pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    println!("Panicked {:?}", panic_info);
    arm_semihosting::exit(1);
}

pub fn panic_handler_should_panic(panic_info: &PanicInfo) -> ! {
    println!("Panicked {:?}", panic_info);
    arm_semihosting::exit(0);
}

pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        let type_name = core::any::type_name::<Self>();
        println!("START {}", type_name);
        self();
        println!("END");
    }
}
