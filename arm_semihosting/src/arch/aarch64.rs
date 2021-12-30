use crate::{HostResult, Operation};

use core::arch::asm;

#[inline]
pub(crate) fn call_host(op: &Operation) -> HostResult {
    let op_code = op.code();
    let args = op.args();
    let mut result: i32;

    unsafe {
        asm!(
            "hlt #0xF000",
            in("w0") op_code,
            in("x1") args,
            lateout("x0") result
        )
    }

    HostResult(result as isize)
}
