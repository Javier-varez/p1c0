use crate::{HostResult, Operation};

#[inline]
pub(crate) fn call_host(_op: &Operation) -> HostResult {
    HostResult(0)
}
