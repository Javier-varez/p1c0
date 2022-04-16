use crate::memory::address::{Address, Validator, VirtualAddress};
use crate::prelude::*;
use core::fmt::Formatter;

#[repr(C)]
struct Frame {
    next: *const Frame,
    lr: *const u8,
}

pub trait Symbolicator {
    fn symbolicate(&self, addr: VirtualAddress) -> Option<(String, usize)>;
}

#[derive(Clone)]
pub struct StackFrameIter<V: Validator, S: Symbolicator> {
    frame_ptr: VirtualAddress,
    validator: V,
    symbolicator: Option<S>,
}

impl<V: Validator, S: Symbolicator> Iterator for StackFrameIter<V, S> {
    type Item = (VirtualAddress, Option<(String, usize)>);

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

        let symbol = if let Some(symbolicator) = &self.symbolicator {
            symbolicator.symbolicate(item)
        } else {
            None
        };

        Some((item, symbol))
    }
}

impl<V: Validator + Clone, S: Symbolicator + Clone> core::fmt::Display for StackFrameIter<V, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let iter_clone = (*self).clone();
        writeln!(f, "Stack trace:")?;
        for (level, (frame, symbol)) in iter_clone.enumerate() {
            if let Some((symbol_name, symbol_offset)) = symbol {
                writeln!(
                    f,
                    "\t[{}] = {} - {} (+0x{:x})",
                    level, frame, symbol_name, symbol_offset
                )?;
            } else {
                writeln!(f, "\t[{}] = {}", level, frame)?;
            }
        }
        Ok(())
    }
}

pub fn stack_frame_iter<V, S>(
    frame_ptr: VirtualAddress,
    validator: V,
    symbolicator: Option<S>,
) -> StackFrameIter<V, S>
where
    V: Validator + Clone,
    S: Symbolicator + Clone,
{
    StackFrameIter {
        frame_ptr,
        validator,
        symbolicator,
    }
}
