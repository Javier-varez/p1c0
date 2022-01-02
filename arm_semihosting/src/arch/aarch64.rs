use crate::Operation;

use core::arch::asm;

#[inline]
pub(crate) unsafe fn call_host_unchecked(op: &mut Operation) -> isize {
    let op_code = op.code();
    let args = op.args();
    let mut result: i32;

    asm!(
        "hlt #0xF000",
        in("w0") op_code,
        in("x1") args,
        lateout("x0") result
    );

    result as isize
}
