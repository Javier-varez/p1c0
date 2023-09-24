#![no_std]
#![cfg_attr(test, no_main)]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, test_runner(test_fwk::runner))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]

use p1c0_kernel::{boot_args::BootArgs, prelude::*};

#[cfg(feature = "coverage")]
use minicov as _;

#[panic_handler]
#[cfg(test)]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

pub fn print_boot_args(boot_args: &BootArgs) {
    log_info!("Boot args:");
    log_info!("\tRevision:           {}", boot_args.revision);
    log_info!("\tVersion:            {}", boot_args.version);
    log_info!("\tVirtual base:       0x{:x}", boot_args.virt_base);
    log_info!("\tPhysical base:      0x{:x}", boot_args.phys_base);
    log_info!("\tMem size:           0x{:x}", boot_args.mem_size);
    log_info!("\tTop of kernel data: 0x{:x}", boot_args.top_of_kernel_data);
    log_info!("\tVideo base:         {:?}", boot_args.boot_video.base);
    log_info!("\tVideo num displays: {}", boot_args.boot_video.display);
    log_info!("\tVideo stride:       {}", boot_args.boot_video.stride);
    log_info!("\tVideo width:        {}", boot_args.boot_video.width);
    log_info!("\tVideo height:       {}", boot_args.boot_video.height);
    log_info!("\tVideo depth:        0x{:x}", boot_args.boot_video.depth);
    log_info!("\tMachine type:       {}", boot_args.machine_type);
    log_info!("\tDevice tree:        {:?}", boot_args.device_tree);
    log_info!("\tDevice tree size:   0x{:x}", boot_args.device_tree_size);
    log_info!("\tBoot flags:         {}", boot_args.boot_flags);
    log_info!("\tMem size actual:    0x{:x}", boot_args.mem_size_actual);
}

#[cfg(feature = "emulator")]
pub fn print_semihosting_caps() {
    let ext = arm_semihosting::load_extensions().unwrap();

    log_debug!("Running emulator with semihosting extensions");
    log_debug!("Extended exit:          {}", ext.supports_extended_exit());
    log_debug!("Stdout-stderr support:  {}", ext.supports_stdout_stderr());
    log_debug!(
        "Cmdline arguments: [{}]",
        arm_semihosting::get_cmd_line().unwrap()
    );
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
    use p1c0_kernel::boot_args::get_boot_args;

    #[test_case]
    fn test_print_boot_args() {
        print_boot_args(get_boot_args());
    }
}
