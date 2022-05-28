#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_fwk::runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(default_alloc_error_handler)]

use p1c0 as _; // needed to link libentry (and _start)

use p1c0_kernel::sync::spinlock::{RwSpinLock, SpinLock};

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    test_fwk::panic_handler(panic_info)
}

#[no_mangle]
pub extern "C" fn kernel_main() {
    test_main();
}

#[test_case]
fn test_spinlock() {
    let spinlock = SpinLock::new(0);
    let lock = spinlock.lock();
    assert!(spinlock.try_lock().is_err());
    drop(lock);
    let _lock = spinlock.try_lock().unwrap();
}

#[test_case]
fn test_rwspinlock() {
    let rwspinlock = RwSpinLock::new(0);
    let rlock1 = rwspinlock.lock_read();
    let rlock2 = rwspinlock.lock_read();
    assert!(rwspinlock.try_lock_write().is_err());
    drop(rlock1);
    assert!(rwspinlock.try_lock_write().is_err());
    drop(rlock2);
    let wlock = rwspinlock.try_lock_write();
    assert!(rwspinlock.try_lock_write().is_err());
    assert!(rwspinlock.try_lock_read().is_err());
    drop(wlock);

    let _rlock1 = rwspinlock.lock_read();
    let _rlock2 = rwspinlock.lock_read();
}

#[test_case]
fn test_spinlock_access_inner_without_locking() {
    let spinlock = SpinLock::new(0);
    let mut did_run = false;
    unsafe {
        spinlock.access_inner_without_locking(|val| {
            assert_eq!(*val, 0);
            did_run = true;
        })
    };
    assert!(did_run);
}
#[test_case]
fn test_rwspinlock_access_inner_without_locking() {
    let spinlock = RwSpinLock::new(0);
    let mut did_run = false;
    unsafe {
        spinlock.access_inner_without_locking(|val| {
            assert_eq!(*val, 0);
            did_run = true;
        })
    };
    assert!(did_run);
}
