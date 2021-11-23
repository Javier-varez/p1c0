#[repr(C)]
pub struct BootVideoArgs {
    pub base: *mut u8,
    pub display: usize,
    pub stride: usize,
    pub width: usize,
    pub height: usize,
    pub depth: usize,
}

#[repr(C)]
pub struct BootArgs {
    pub revision: u16,
    pub version: u16,
    pub virt_base: usize,
    pub phys_base: usize,
    pub mem_size: usize,
    pub top_of_kernel_data: usize,
    pub boot_video: BootVideoArgs,
    pub machine_type: u32,
    pub device_tree: *mut u8,
    pub device_tree_size: usize,
    pub cmdline: [u8; 608],
    pub boot_flags: u64,
    pub mem_size_actual: u64,
}
