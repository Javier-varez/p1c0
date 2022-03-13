use core::sync::atomic;
use cortex_a::{asm::barrier, registers::DAIF};
use tock_registers::interfaces::{Readable, Writeable};

static CRITICAL_NESTING: atomic::AtomicU32 = atomic::AtomicU32::new(0);
static mut SAVED_DAIF: u64 = 0;

use core::cell::UnsafeCell;

#[derive(Debug)]
pub enum Error {
    WouldBlock,
}

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

    /// # Safety
    ///   In order for this to be safe you need to manually ensure that there is no other thread
    ///   that could be accessing the object inside the lock
    pub unsafe fn access_inner_without_locking(&self, mut f: impl FnMut(&mut T)) {
        f(&mut *self.data.get())
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        loop {
            if let Ok(guard) = self.try_lock() {
                return guard;
            }
        }
    }

    pub fn try_lock(&self) -> Result<SpinLockGuard<'_, T>, Error> {
        let saved_daif = DAIF.get();

        DAIF.write(DAIF::D::Masked + DAIF::I::Masked + DAIF::A::Masked + DAIF::F::Masked);
        unsafe { barrier::dsb(barrier::ISHST) };

        match self.lock.compare_exchange(
            false,
            true,
            atomic::Ordering::Acquire,
            atomic::Ordering::Relaxed,
        ) {
            Ok(_) => {
                let prev_nesting = CRITICAL_NESTING.fetch_add(1, atomic::Ordering::Acquire);
                if prev_nesting == core::u32::MAX {
                    panic!("We have reached the maximum value for CRITICAL_NESTING. This is MOST LIKELY a bug in user code");
                } else if prev_nesting == 0 {
                    // Save the daif value for later when it is unlocked
                    unsafe { SAVED_DAIF = saved_daif };
                }

                Ok(SpinLockGuard {
                    lock: self,
                    data: unsafe { &mut *self.data.get() },
                })
            }
            Err(_) => {
                // Restore daif. This gives the processor a chance to run some
                // interrupt/exceptions while looping
                DAIF.set(saved_daif);

                // Add a barrier here to ensure that subsequent memory accesses really execute
                // out of the critical section
                unsafe { barrier::dsb(barrier::ISHST) };
                Err(Error::WouldBlock)
            }
        }
    }

    fn unlock(&self) {
        assert!(self.lock.swap(false, atomic::Ordering::Release));

        let prev_nesting = CRITICAL_NESTING.fetch_sub(1, atomic::Ordering::Release);
        if prev_nesting == 1 {
            // Add a barrier here to ensure that memory accesses finish before enabling exceptions
            unsafe { barrier::dsb(barrier::ISHST) };

            // Restore daif settings
            unsafe { DAIF.set(SAVED_DAIF) };
        }
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
