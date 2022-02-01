#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]

use m1::boot_args::BootArgs;
use m1::println;

#[panic_handler]
#[cfg(test)]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

pub fn print_boot_args(boot_args: &BootArgs) {
    println!("===== BOOT ARGS =====");
    println!("Revision:           {}", boot_args.revision);
    println!("Version:            {}", boot_args.version);
    println!("Virtual base:       0x{:x}", boot_args.virt_base);
    println!("Physical base:      0x{:x}", boot_args.phys_base);
    println!("Mem size:           0x{:x}", boot_args.mem_size);
    println!("Top of kernel data: 0x{:x}", boot_args.top_of_kernel_data);
    println!("Video base:         {:?}", boot_args.boot_video.base);
    println!("Video num displays: {}", boot_args.boot_video.display);
    println!("Video stride:       {}", boot_args.boot_video.stride);
    println!("Video width:        {}", boot_args.boot_video.width);
    println!("Video height:       {}", boot_args.boot_video.height);
    println!("Video depth:        0x{:x}", boot_args.boot_video.depth);
    println!("Machine type:       {}", boot_args.machine_type);
    println!("Device tree:        {:?}", boot_args.device_tree);
    println!("Device tree size:   0x{:x}", boot_args.device_tree_size);
    println!("Boot flags:         {}", boot_args.boot_flags);
    println!("Mem size actual:    0x{:x}", boot_args.mem_size_actual);
    println!();
}

#[cfg(feature = "emulator")]
pub fn print_semihosting_caps() {
    let ext = arm_semihosting::load_extensions().unwrap();

    println!("Running emulator with semihosting extensions:");
    println!("Extended exit:          {}", ext.supports_extended_exit());
    println!("Stdout-stderr support:  {}", ext.supports_stdout_stderr());
    println!(
        "Cmdline arguments: [{}]",
        arm_semihosting::get_cmd_line().unwrap()
    );
    println!();
}

#[no_mangle]
#[cfg(test)]
pub extern "C" fn kernel_main() {
    #[cfg(test)]
    test_main();
}

#[cfg(test)]
mod tests {
    use super::print_boot_args;
    use m1::boot_args::get_boot_args;
    use m1::drivers::generic_timer::get_timer;

    #[test_case]
    fn test_print_boot_args() {
        print_boot_args(get_boot_args());
    }

    #[test_case]
    fn test_system_timer() {
        let timer = get_timer();
        let resolution = timer.resolution();
        crate::println!("Timer resolution is {}", resolution);
        let old_ticks = timer.ticks();
        crate::println!("Timer ticks is {}", old_ticks);
        let new_ticks = timer.ticks();
        crate::println!("Timer ticks is {}", new_ticks);
        assert!(new_ticks > old_ticks);
    }
}
