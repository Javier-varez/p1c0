use crate::Operation;

#[inline]
pub(crate) unsafe fn call_host_unchecked(_op: &mut Operation) -> isize {
    0
}
