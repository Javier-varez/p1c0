#![no_std]

use arm_semihosting::{print, println};
use core::ops::Fn;

use ansi_rgb::{cyan_blue, green_cyan, red, Foreground};

use core::panic::PanicInfo;

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    Fail,
    Success,
}

pub fn runner(tests: &[&dyn Testable]) {
    println!("{}", "Starting test execution".fg(cyan_blue()));
    tests.iter().for_each(|test| test.run());
    println!("{}", "Done with test execution".fg(green_cyan()));
    arm_semihosting::exit(0);
}

pub fn runner_should_panic(tests: &[&dyn Testable]) {
    println!("{}", "Starting test execution".fg(cyan_blue()));
    tests.iter().for_each(|test| test.run());
    println!("{}", "Done with test execution".fg(red()));
    arm_semihosting::exit(1);
}

pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    println!("{} {:?}", "Panicked at:".fg(red()), panic_info);
    arm_semihosting::exit(1);
}

pub fn panic_handler_should_panic(panic_info: &PanicInfo) -> ! {
    println!("{} {:?}", "Expected panic at:".fg(green_cyan()), panic_info);
    arm_semihosting::exit(0);
}

pub fn finish_with_status(status: Status) -> ! {
    if status == Status::Success {
        println!("{}", "Done with test execution".fg(green_cyan()));
        arm_semihosting::exit(0);
    } else {
        println!("{}", "Test failed".fg(red()));
        arm_semihosting::exit(1);
    }
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
        print!(
            "{} {} ... ",
            "Running test:".fg(cyan_blue()),
            type_name.fg(cyan_blue())
        );
        self();
        println!("{}", "ok".fg(green_cyan()));
    }
}
