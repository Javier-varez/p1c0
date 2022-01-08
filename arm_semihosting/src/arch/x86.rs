use crate::Operation;

#[inline]
pub(crate) unsafe fn call_host_unchecked(op: &mut Operation) -> isize {
    let _ = op.args();
    let _ = op.code();
    0
}
