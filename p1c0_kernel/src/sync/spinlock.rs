use core::sync::atomic;
use cortex_a::{asm::barrier, registers::DAIF};
use tock_registers::interfaces::Writeable;

static CRITICAL_NESTING: atomic::AtomicU32 = atomic::AtomicU32::new(0);

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

        let prev_nesting = CRITICAL_NESTING.fetch_add(1, atomic::Ordering::Acquire);
        if prev_nesting == core::u32::MAX {
            panic!("We have reached the maximum value for CRITICAL_NESTING. This is MOST LIKELY a bug in user code");
        } else if prev_nesting == 0 {
            // Disable exceptions here because they were enabled (CRITICAL_NESTING was 0)
            DAIF.write(DAIF::D::Masked + DAIF::I::Masked + DAIF::A::Masked + DAIF::F::Masked);

            // Add a barrier here to ensure that subsequent memory accesses really execute within
            // the critical section
            unsafe { barrier::dsb(barrier::ISHST) };
        }

        SpinLockGuard {
            lock: self,
            data: unsafe { &mut *self.data.get() },
        }
    }

    fn unlock(&self) {
        let prev_nesting = CRITICAL_NESTING.fetch_sub(1, atomic::Ordering::Release);
        if prev_nesting == core::u32::MAX {
            panic!("We have reached the maximum value for CRITICAL_NESTING. This is MOST LIKELY a bug in user code");
        } else if prev_nesting == 1 {
            // Add a barrier here to ensure that memory accesses finish before enabling exceptions
            unsafe { barrier::dsb(barrier::ISHST) };

            // Enable exceptions here because they were disabled (CRITICAL_NESTING was 1)
            DAIF.write(
                DAIF::D::Unmasked + DAIF::I::Unmasked + DAIF::A::Unmasked + DAIF::F::Unmasked,
            );
        }

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
