#![allow(dead_code)]
pub mod aligned_vec;
pub mod intrusive_list;
pub mod ring_buffer;

extern crate alloc;
use alloc::boxed::Box;

#[cfg(not(test))]
use crate::println;

/// This is a type that owns a pointer and cannot be dropped. If it is dropped it logs the problem.
/// Instead, the pointer should be freed and used in a different manner (e.g: using it
/// to construct a Box from a valid pointer).
///
/// The idea is to catch memory leaks and not just use a raw pointer which doesn't indicate any
/// ownership.
///
pub struct OwnedMutPtr<T> {
    inner: *mut T,
}

pub struct OwnedPtr<T> {
    inner: *const T,
}

impl<T> OwnedMutPtr<T> {
    pub fn new_from_box(ptr: Box<T>) -> Self {
        Self {
            inner: Box::leak(ptr),
        }
    }

    pub unsafe fn new_from_raw(ptr: *mut T) -> Self {
        Self { inner: ptr }
    }

    pub fn leak(self) -> *mut T {
        let item = core::mem::ManuallyDrop::new(self);
        item.inner
    }

    /// # Safety
    /// Should only be called if the pointer was originally allocated with Box using the global
    /// allocator
    #[must_use]
    pub unsafe fn into_box(self) -> Box<T> {
        Box::from_raw(self.leak())
    }
}

impl<T> OwnedPtr<T> {
    pub fn new_from_box(ptr: Box<T>) -> Self {
        Self {
            inner: Box::leak(ptr),
        }
    }

    pub unsafe fn new_from_raw(ptr: *mut T) -> Self {
        Self { inner: ptr }
    }

    pub fn leak(self) -> *const T {
        let item = core::mem::ManuallyDrop::new(self);
        item.inner
    }

    /// # Safety
    /// Should only be called if the pointer was originally allocated with Box using the global
    /// allocator
    #[must_use]
    pub unsafe fn into_box(self) -> Box<T> {
        Box::from_raw(self.leak() as *mut _)
    }
}

impl<T> core::ops::Deref for OwnedMutPtr<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl<T> core::ops::DerefMut for OwnedMutPtr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner }
    }
}

impl<T> core::ops::Deref for OwnedPtr<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner }
    }
}

impl<T> Drop for OwnedMutPtr<T> {
    fn drop(&mut self) {
        // TODO(javier-varez): Print backtrace here when available
        println!(
            "Attempted to drop an OwnedMutPtr<{}> with address {:?}",
            core::any::type_name::<T>(),
            self.inner,
        );
    }
}

impl<T> Drop for OwnedPtr<T> {
    fn drop(&mut self) {
        // TODO(javier-varez): Print backtrace here when available
        println!(
            "Attempted to drop an OwnedPtr<{}> with address {:?}",
            core::any::type_name::<T>(),
            self.inner
        );
    }
}
