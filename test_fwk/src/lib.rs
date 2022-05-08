#![no_std]

use arm_semihosting::{print, println};
use core::{
    ops::Fn,
    sync::atomic::{AtomicBool, Ordering},
};

use ansi_rgb::{cyan_blue, green_cyan, red, Foreground};

use core::panic::PanicInfo;

#[cfg(feature = "coverage")]
use minicov as _;

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    Fail,
    Success,
}

fn exit_and_collect_coverage(status: Status) -> ! {
    #[cfg(feature = "coverage")]
    {
        // Get the command line and use the name of the executable for the coverage file
        let cmdline = arm_semihosting::get_cmd_line().unwrap();
        if !cmdline.is_empty() {
            println!("Saving coverage as: {}", cmdline);
            let coverage = minicov::capture_coverage();
            let mut file = match arm_semihosting::io::create(
                &cmdline,
                arm_semihosting::io::AccessType::Binary,
            ) {
                Ok(f) => f,
                Err(_) => {
                    println!("Error opening coverage file");
                    arm_semihosting::exit(1);
                }
            };

            if let Err(_) = file.write(&coverage) {
                println!("Error saving coverage data");
            }
        }
    }

    let exit_code = if status == Status::Success { 0 } else { 1 };
    arm_semihosting::exit(exit_code);
}

pub fn runner(tests: &[&dyn Testable]) {
    println!("{}", "Starting test execution".fg(cyan_blue()));
    tests.iter().for_each(|test| test.run());
    finish_with_status(Status::Success);
}

pub fn runner_should_panic(tests: &[&dyn Testable]) {
    println!("{}", "Starting test execution".fg(cyan_blue()));
    tests.iter().for_each(|test| test.run());
    finish_with_status(Status::Fail);
}

pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    static ALREADY_PANICKED: AtomicBool = AtomicBool::new(false);
    if ALREADY_PANICKED.load(Ordering::Relaxed) {
        println!("{}", "Panicked while panicking".fg(red()));
        arm_semihosting::exit(1);
    }
    ALREADY_PANICKED.store(true, Ordering::Relaxed);

    println!("{} {:?}", "Panicked at:".fg(red()), panic_info);
    finish_with_status(Status::Fail);
}

pub fn panic_handler_should_panic(panic_info: &PanicInfo) -> ! {
    static ALREADY_PANICKED: AtomicBool = AtomicBool::new(false);
    if ALREADY_PANICKED.load(Ordering::Relaxed) {
        println!("{}", "Panicked while panicking".fg(red()));
        arm_semihosting::exit(1);
    }

    ALREADY_PANICKED.store(true, Ordering::Relaxed);
    println!("{} {:?}", "Expected panic at:".fg(green_cyan()), panic_info);
    finish_with_status(Status::Success);
}

pub fn finish_with_status(status: Status) -> ! {
    if status == Status::Success {
        println!("{}", "Done with test execution".fg(green_cyan()));
    } else {
        println!("{}", "Test failed".fg(red()));
    }
    exit_and_collect_coverage(status);
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
