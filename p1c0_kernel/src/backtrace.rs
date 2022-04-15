use crate::memory::address::{Address, Validator, VirtualAddress};
use core::fmt::Formatter;

#[repr(C)]
struct Frame {
    next: *const Frame,
    lr: *const u8,
}

#[derive(Clone)]
pub struct StackFrameIter<V: Validator> {
    frame_ptr: VirtualAddress,
    validator: V,
}

impl<V: Validator> Iterator for StackFrameIter<V> {
    type Item = VirtualAddress;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.validator.is_valid(self.frame_ptr) {
            return None;
        }

        let frame_ptr = self.frame_ptr.as_ptr() as *const Frame;

        // # Safety: This should be safe because it is within the validated range
        let item = VirtualAddress::new_unaligned(unsafe { (*frame_ptr).lr });

        self.frame_ptr = VirtualAddress::new_unaligned(unsafe { (*frame_ptr).next } as *const _);

        // We hit the end on nullptr
        if item.as_ptr().is_null() {
            return None;
        }

        Some(item)
    }
}

impl<V: Validator + Clone> core::fmt::Display for StackFrameIter<V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let iter_clone = (*self).clone();
        writeln!(f, "Stack trace:")?;
        for (level, frame) in iter_clone.enumerate() {
            writeln!(f, "\t[{}] = {}", level, frame)?;
        }
        Ok(())
    }
}

pub fn stack_frame_iter(
    frame_ptr: VirtualAddress,
    validator: impl Validator + Clone,
) -> StackFrameIter<impl Validator + Clone> {
    StackFrameIter {
        frame_ptr,
        validator,
    }
}
