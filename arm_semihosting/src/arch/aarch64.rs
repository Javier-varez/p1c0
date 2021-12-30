use crate::{HostResult, Operation};

use core::arch::asm;

#[inline]
pub fn call_host(op: &Operation) -> HostResult {
    let op_code = op.code();
    let args = op.args();
    let mut result: usize;

    unsafe {
        asm!(
            "hlt #0xF000",
            in("w0") op_code,
            in("x1") args,
            lateout("x0") result
        )
    }

    HostResult(result)
}
