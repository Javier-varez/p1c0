use core::sync::atomic;
use cortex_a::{asm::barrier, registers::DAIF};
use tock_registers::interfaces::{Readable, Writeable};

use core::cell::UnsafeCell;

static CRITICAL_NESTING: atomic::AtomicU32 = atomic::AtomicU32::new(0);
static mut SAVED_DAIF: u64 = 0;

#[derive(Debug)]
pub enum Error {
    WouldBlock,
}

type Result<T> = core::result::Result<T, Error>;

fn get_then_mask_daif() -> u64 {
    let saved_daif = DAIF.get();

    DAIF.write(DAIF::D::Masked + DAIF::I::Masked + DAIF::A::Masked + DAIF::F::Masked);
    unsafe { barrier::dsb(barrier::ISHST) };
    saved_daif
}

fn restore_saved_daif(saved_daif: u64) {
    // Restore daif. This gives the processor a chance to run some
    // interrupt/exceptions while looping
    DAIF.set(saved_daif);

    // Add a barrier here to ensure that subsequent memory accesses really execute
    // out of the critical section
    unsafe { barrier::dsb(barrier::ISHST) };
}

fn increment_critical_nesting(saved_daif: u64) {
    assert_eq!(DAIF.read(DAIF::D), 1);
    assert_eq!(DAIF.read(DAIF::A), 1);
    assert_eq!(DAIF.read(DAIF::I), 1);
    assert_eq!(DAIF.read(DAIF::F), 1);

    let prev_nesting = CRITICAL_NESTING.fetch_add(1, atomic::Ordering::Acquire);
    if prev_nesting == core::u32::MAX {
        panic!("We have reached the maximum value for CRITICAL_NESTING. This is MOST LIKELY a bug in user code");
    } else if prev_nesting == 0 {
        // Save the daif value for later when it is unlocked
        unsafe { SAVED_DAIF = saved_daif };
    }
}

fn decrement_critical_nesting() {
    let prev_nesting = CRITICAL_NESTING.fetch_sub(1, atomic::Ordering::Release);
    if prev_nesting == 1 {
        // Add a barrier here to ensure that memory accesses finish before enabling exceptions
        unsafe { barrier::dsb(barrier::ISHST) };

        // Restore daif settings
        unsafe { DAIF.set(SAVED_DAIF) };
    }
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

    pub fn try_lock(&self) -> Result<SpinLockGuard<'_, T>> {
        let saved_daif = get_then_mask_daif();

        match self.lock.compare_exchange(
            false,
            true,
            atomic::Ordering::Acquire,
            atomic::Ordering::Relaxed,
        ) {
            Ok(_) => {
                increment_critical_nesting(saved_daif);

                Ok(SpinLockGuard {
                    lock: self,
                    data: unsafe { &mut *self.data.get() },
                })
            }
            Err(_) => {
                restore_saved_daif(saved_daif);

                Err(Error::WouldBlock)
            }
        }
    }

    fn unlock(&self) {
        assert!(self.lock.swap(false, atomic::Ordering::Release));

        decrement_critical_nesting();
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

pub struct RwSpinLock<T> {
    lock: atomic::AtomicU32,
    data: UnsafeCell<T>,
}

impl<T> RwSpinLock<T> {
    const WRITE_LOCK_FLAG: u32 = 1;
    const NUM_READERS_OFFSET: u32 = 1;
    const NUM_READERS_MASK: u32 = 0xFFFFFFFE;

    pub const fn new(data: T) -> Self {
        Self {
            lock: atomic::AtomicU32::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// # Safety
    ///   In order for this to be safe you need to manually ensure that there is no other thread
    ///   that could be accessing the object inside the lock
    pub unsafe fn access_inner_without_locking(&self, mut f: impl FnMut(&mut T)) {
        f(&mut *self.data.get())
    }

    pub fn try_lock_read(&self) -> Result<ReadGuard<'_, T>> {
        loop {
            let saved_daif = get_then_mask_daif();

            let lock = self.lock.load(atomic::Ordering::Relaxed);
            if (lock & Self::WRITE_LOCK_FLAG) != 0 {
                return Err(Error::WouldBlock);
            }

            // We cannot lock more than 2 giga-times
            assert_ne!(lock & Self::NUM_READERS_MASK, Self::NUM_READERS_MASK);

            let affects_nesting = (lock & Self::NUM_READERS_MASK) == 0;

            let new_lock = lock + (1 << Self::NUM_READERS_OFFSET);

            match self.lock.compare_exchange(
                lock,
                new_lock,
                atomic::Ordering::Acquire,
                atomic::Ordering::Relaxed,
            ) {
                Ok(_) => {
                    if affects_nesting {
                        increment_critical_nesting(saved_daif);
                    }

                    return Ok(ReadGuard {
                        lock: self,
                        data: unsafe { &*self.data.get() },
                    });
                }
                Err(_) => {
                    restore_saved_daif(saved_daif);
                }
            }
        }
    }

    fn read_unlock(&self) {
        let mut affects_critical_nesting;

        loop {
            affects_critical_nesting = false;

            let lock = self.lock.load(atomic::Ordering::Relaxed);

            // It must not be locked for writing
            assert_eq!(lock & Self::WRITE_LOCK_FLAG, 0);

            // If all readers have been unlocked already this is a BUG
            assert_ne!(lock & Self::NUM_READERS_MASK, 0);

            if (lock >> Self::NUM_READERS_OFFSET) == 1 {
                affects_critical_nesting = true;
            }

            let new_lock = lock - (1 << Self::NUM_READERS_OFFSET);

            if self
                .lock
                .compare_exchange(
                    lock,
                    new_lock,
                    atomic::Ordering::Acquire,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }

        if affects_critical_nesting {
            decrement_critical_nesting();
        }
    }

    pub fn try_lock_write(&self) -> Result<WriteGuard<'_, T>> {
        loop {
            let saved_daif = get_then_mask_daif();

            let lock = self.lock.load(atomic::Ordering::Relaxed);
            if ((lock & Self::WRITE_LOCK_FLAG) != 0) || ((lock & Self::NUM_READERS_MASK) != 0) {
                return Err(Error::WouldBlock);
            }

            let new_lock = lock | Self::WRITE_LOCK_FLAG;

            match self.lock.compare_exchange_weak(
                lock,
                new_lock,
                atomic::Ordering::Acquire,
                atomic::Ordering::Relaxed,
            ) {
                Ok(_) => {
                    increment_critical_nesting(saved_daif);

                    return Ok(WriteGuard {
                        lock: self,
                        data: unsafe { &mut *self.data.get() },
                    });
                }
                Err(_) => {
                    restore_saved_daif(saved_daif);
                }
            }
        }
    }

    fn write_unlock(&self) {
        loop {
            let lock = self.lock.load(atomic::Ordering::Relaxed);

            // It must be locked for writing
            assert_eq!(lock & Self::WRITE_LOCK_FLAG, 1);

            // It must not be locked for reading
            assert_eq!(lock & Self::NUM_READERS_MASK, 0);

            let new_lock = lock & !Self::WRITE_LOCK_FLAG;

            if self
                .lock
                .compare_exchange_weak(
                    lock,
                    new_lock,
                    atomic::Ordering::Acquire,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                break;
            }
        }

        decrement_critical_nesting();
    }

    pub fn lock_read(&self) -> ReadGuard<'_, T> {
        loop {
            if let Ok(guard) = self.try_lock_read() {
                return guard;
            }
        }
    }

    pub fn lock_write(&self) -> WriteGuard<'_, T> {
        loop {
            if let Ok(guard) = self.try_lock_write() {
                return guard;
            }
        }
    }
}

unsafe impl<T> Send for RwSpinLock<T> {}

unsafe impl<T> Sync for RwSpinLock<T> {}

pub struct ReadGuard<'a, T> {
    lock: &'a RwSpinLock<T>,
    data: &'a T,
}

impl<'a, T> core::ops::Deref for ReadGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> Drop for ReadGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.read_unlock();
    }
}

pub struct WriteGuard<'a, T> {
    lock: &'a RwSpinLock<T>,
    data: &'a mut T,
}

impl<'a, T> core::ops::Deref for WriteGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T> core::ops::DerefMut for WriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'a, T> Drop for WriteGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.write_unlock();
    }
}
