#[repr(C)]
#[derive(Clone, Debug)]
pub struct BootVideoArgs {
    pub base: *mut u8,
    pub display: usize,
    pub stride: usize,
    pub width: usize,
    pub height: usize,
    pub depth: usize,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct BootArgs {
    pub revision: u16,
    pub version: u16,
    pub virt_base: usize,
    pub phys_base: usize,
    pub mem_size: usize,
    pub top_of_kernel_data: usize,
    pub boot_video: BootVideoArgs,
    pub machine_type: u32,
    pub device_tree: *const u8,
    pub device_tree_size: u32,
    pub cmdline: [u8; 608],
    pub boot_flags: u64,
    pub mem_size_actual: u64,
}

static mut BOOT_ARGS: Option<BootArgs> = None;

/// Assumes that set_boot_args has been called and panics if the option is None
pub fn get_boot_args() -> &'static BootArgs {
    unsafe { BOOT_ARGS.as_ref().expect("Boot args are set") }
}

/// Must be called by the init code of the processor.
/// SAFETY
///   This shall only be called right after booting where no-one has already accessed the boot
///   arguments and there is only one thread running
pub(crate) unsafe fn set_boot_args(boot_args: &BootArgs) {
    BOOT_ARGS.replace(boot_args.clone());
}
