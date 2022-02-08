use core::sync::atomic;

use core::cell::UnsafeCell;

pub struct SpinLock<T> {
    lock: atomic::AtomicBool,
    data: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            lock: atomic::AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        while self
            .lock
            .compare_exchange_weak(
                false,
                true,
                atomic::Ordering::Acquire,
                atomic::Ordering::Relaxed,
            )
            .is_err()
        {}

        SpinLockGuard {
            lock: self,
            data: unsafe { &mut *self.data.get() },
        }
    }

    fn unlock(&self) {
        self.lock.swap(false, atomic::Ordering::Release);
    }
}

unsafe impl<T> Send for SpinLock<T> {}
unsafe impl<T> Sync for SpinLock<T> {}

pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    data: &'a mut T,
}

impl<'a, T> core::ops::Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> core::ops::DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'a, T> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}
