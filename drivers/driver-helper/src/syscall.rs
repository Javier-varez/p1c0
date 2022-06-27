pub fn print_str(str: &str) {
    unsafe {
        core::arch::asm!(concat!("svc ", 6),
                         in("x0") str.as_ptr(),
                         in("x1") str.len(),
        );
    }
}

pub fn exit(code: u64) -> ! {
    unsafe {
        core::arch::asm!(concat!("svc ", 8),
                         in("x0") code,
        );
    }
    unreachable!();
}
