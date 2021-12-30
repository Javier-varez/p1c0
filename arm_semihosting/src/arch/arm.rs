use crate::{HostResult, Operation};

use core::arch::asm;

#[inline]
#[cfg_attr(target_cpu = "arm", instruction_set(arm::a32))]
pub(crate) fn call_host(op: &Operation) -> HostResult {
    let op_code = op.code();
    let args = op.args();
    let mut result: isize;

    unsafe {
        asm!(
            "svc #0x123456",
            inlateout("r0") op_code => result,
            in("r1") args,
        )
    }

    HostResult(result)
}
