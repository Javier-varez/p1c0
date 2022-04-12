use core::fmt::Formatter;

#[repr(C)]
pub struct Frame {
    next: *const Frame,
    lr: *const (),
}

#[derive(Clone)]
pub struct StackFrameIter(*const Frame);

impl Iterator for StackFrameIter {
    type Item = *const ();
    fn next(&mut self) -> Option<Self::Item> {
        if !self.0.is_null() {
            let item = unsafe { (*self.0).lr };
            unsafe { self.0 = (*self.0).next };
            if item.is_null() {
                return None;
            }
            return Some(item);
        }
        None
    }
}

impl core::fmt::Display for StackFrameIter {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut iter_clone = self.clone();
        let mut level = 0;
        write!(f, "Stack trace:\n")?;
        while let Some(frame) = iter_clone.next() {
            write!(f, "\t[{}] = {:?}\n", level, frame)?;
            level += 1;
        }
        Ok(())
    }
}

pub unsafe fn stack_frame_iter(fp: *const Frame) -> StackFrameIter {
    StackFrameIter(fp)
}
