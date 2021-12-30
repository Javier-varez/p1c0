use crate::{HostResult, Operation};

#[inline]
pub fn call_host(_op: &Operation) -> HostResult {
    HostResult(0)
}
