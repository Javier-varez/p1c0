use crate::sync::spinlock::SpinLock;
use crate::{
    arch::exceptions::ExceptionContext, log_info, log_warning, process, thread, thread::current_pid,
};

macro_rules! gen_syscall_caller {
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ()
    ) => {
        pub fn $syscall_fn_name() {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                core::arch::asm!(concat!("svc ", $syscall_idx),
                );
            }
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ($arg0_ty: ty)
    ) => {
        #[cfg_attr(test, allow(unused_variables))]
        pub fn $syscall_fn_name(arg0: $arg0_ty) {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 in("x0") arg0 as u64
                );
            }
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty)
    ) => {
        #[cfg_attr(test, allow(unused_variables))]
        pub fn $syscall_fn_name(arg0: $arg0_ty, arg1: $arg1_ty) {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                #[cfg(not(test))]
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 in("x0") arg0 as u64,
                                 in("x1") arg1 as u64,
                );
            }
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty)
    ) => {
        #[cfg_attr(test, allow(unused_variables))]
        pub fn $syscall_fn_name(arg0: $arg0_ty, arg1: $arg1_ty, arg2: $arg2_ty) {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 in("x0") arg0 as u64,
                                 in("x1") arg1 as u64,
                                 in("x2") arg2 as u64,
                );
            }
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty, $arg3_ty: ty)
    ) => {
        #[cfg_attr(test, allow(unused_variables))]
        pub fn $syscall_fn_name(arg0: $arg0_ty,
                                arg1: $arg1_ty,
                                arg2: $arg2_ty,
                                arg3: $arg3_ty) {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 in("x0") arg0 as u64,
                                 in("x1") arg1 as u64,
                                 in("x2") arg2 as u64,
                                 in("x3") arg3 as u64,
                );
            }
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        () -> $ret_ty: ty
    ) => {
        pub fn $syscall_fn_name() -> $ret_ty {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                let mut result: $ret_ty;
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 out("x0") result,
                );
                result
            }

            #[cfg(any(not(target_arch = "aarch64"), test))]
            0
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ($arg0_ty: ty) -> $ret_ty: ty
    ) => {
        #[cfg_attr(test, allow(unused_variables))]
        pub fn $syscall_fn_name(arg0: $arg0_ty) -> $ret_ty {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                let mut result: $ret_ty;
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 in("x0") arg0 as u64,
                                 lateout("x0") result,
                );
                result
            }

            #[cfg(any(not(target_arch = "aarch64"), test))]
            0
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty) -> $ret_ty: ty
    ) => {
        #[cfg_attr(test, allow(unused_variables))]
        pub fn $syscall_fn_name(arg0: $arg0_ty, arg1: $arg1_ty) -> $ret_ty {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                let mut result: $ret_ty;
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 in("x0") arg0 as u64,
                                 in("x1") arg1 as u64,
                                 lateout("x0") result,
                );
                result
            }

            #[cfg(any(not(target_arch = "aarch64"), test))]
            0
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty) -> $ret_ty: ty
    ) => {
        #[cfg_attr(test, allow(unused_variables))]
        pub fn $syscall_fn_name(arg0: $arg0_ty, arg1: $arg1_ty, arg2: $arg2_ty) -> $ret_ty {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                let mut result: $ret_ty;
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 in("x0") arg0 as u64,
                                 in("x1") arg1 as u64,
                                 in("x2") arg2 as u64,
                                 lateout("x0") result,
                );
                result
            }

            #[cfg(any(not(target_arch = "aarch64"), test))]
            0
        }
    };
    (
        $syscall_idx: literal,
        $syscall_fn_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty, $arg3_ty: ty) -> $ret_ty: ty
    ) => {
        #[cfg_attr(test, allow(unused_variables))]
        pub fn $syscall_fn_name(arg0: $arg0_ty,
                                arg1: $arg1_ty,
                                arg2: $arg2_ty,
                                arg3: $arg3_ty) -> $ret_ty {
            #[cfg(all(target_arch = "aarch64", not(test)))]
            unsafe {
                let mut result: $ret_ty;
                core::arch::asm!(concat!("svc ", $syscall_idx),
                                 in("x0") arg0 as u64,
                                 in("x1") arg1 as u64,
                                 in("x2") arg2 as u64,
                                 in("x3") arg3 as u64,
                                 lateout("x0") result,
                );
                result
            }

            #[cfg(any(not(target_arch = "aarch64"), test))]
            0
        }
    };
}

macro_rules! call_syscall_hdlr {
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ()
    ) => {
        $syscall_hdlr_name($context);
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty)
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        $syscall_hdlr_name($context, arg0);
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty)
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        $syscall_hdlr_name($context, arg0, arg1);
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty)
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        let arg2 = $context.gpr[2] as $arg2_ty;
        $syscall_hdlr_name($context, arg0, arg1, arg2);
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty, $arg3_ty: ty)
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        let arg2 = $context.gpr[2] as $arg2_ty;
        let arg3 = $context.gpr[3] as $arg3_ty;
        $syscall_hdlr_name($context, arg0, arg1, arg2, arg3);
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        () -> $ret_ty: ty
    ) => {
        let result = $syscall_hdlr_name($context);
        $context.gpr[0] = result as u64;
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty) -> $ret_ty: ty
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let result = $syscall_hdlr_name($context, arg0);
        $context.gpr[0] = result as u64;
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty) -> $ret_ty: ty
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        let result = $syscall_hdlr_name($context, arg0, arg1);
        $context.gpr[0] = result as u64;
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty) -> $ret_ty: ty
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        let arg2 = $context.gpr[2] as $arg2_ty;
        let result = $syscall_hdlr_name($context, arg0, arg1, arg2);
        $context.gpr[0] = result as u64;
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty, $arg3_ty: ty) -> $ret_ty: ty
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        let arg2 = $context.gpr[2] as $arg2_ty;
        let arg3 = $context.gpr[3] as $arg3_ty;
        let result = $syscall_hdlr_name($context, arg0, arg1, arg2, arg3);
        $context.gpr[0] = result as u64;
    };
}

macro_rules! define_syscalls {
    (
        $(
            [
                $syscall_idx: literal,
                $syscall_name: ident,
                $syscall_fn_name: ident,
                $syscall_hdlr_name: ident,
                ( $($argv_ty: ty),* ) $(-> $ret_ty: ty)?
            ],
        )+
    ) => {
        pub enum Syscall {
            $($syscall_name = $syscall_idx),*
        }

        impl TryFrom<u32> for Syscall {
            type Error = Error;
            fn try_from(value: u32) -> Result<Self, Self::Error> {
                match value {
                    $($syscall_idx => Ok(Syscall::$syscall_name),)*
                    _ => Err(Error::UnknownSyscall(value)),
                }
            }
        }

        impl Syscall {
            $(
                gen_syscall_caller!(
                    $syscall_idx,
                    $syscall_fn_name,
                    ($($argv_ty),*) $(-> $ret_ty)*
                );
            )*
        }

        pub(crate) fn syscall_handler(imm: u32, cx: &mut ExceptionContext) {
            match imm.try_into() {
                $(
                    Ok(Syscall::$syscall_name) => {
                        call_syscall_hdlr!(cx, $syscall_hdlr_name, ($($argv_ty),*) $(-> $ret_ty)*);
                    }
                )*
                Err(Error::UnknownSyscall(id)) => {
                    // TODO(Javier-varez): We should kill the process here or panic if this was the kernel
                    panic!("BUG: Received unknown syscall from user process: {}", id);
                }
            };
        }

    };
}

define_syscalls!(
    [0, Noop, noop, handle_noop, ()],
    [1, Reboot, reboot, handle_reboot, ()],
    [2, Sleep, sleep_us, handle_sleep_us, (u64)],
    [3, Yield, yield_exec, handle_yield_exec, ()],
    [4, ThreadExit, thread_exit, handle_thread_exit, ()],
    [5, ThreadJoin, thread_join, handle_thread_join, (u64)],
    [6, PutString, puts, handle_puts, (*const u8, usize)],
    [7, WaitPid, wait_pid, handle_wait_pid, (u64) -> u64],
    [8, Exit, exit, handle_exit, (u64)],
    [0x8000, Multiply, multiply, handle_multiply, (u32, u32) -> u32],
);

pub enum Error {
    UnknownSyscall(u32),
}

fn handle_noop(_cx: &mut ExceptionContext) {
    log_info!("Syscall Noop");
}

fn handle_reboot(_cx: &mut ExceptionContext) {
    log_warning!("Syscall Reboot - Rebooting computer");
    unsafe {
        crate::print::force_flush();
    }

    // We hang here never servicing the WDT again, causing a reboot
    loop {
        cortex_a::asm::wfi();
    }
}

fn handle_multiply(_cx: &mut ExceptionContext, a: u32, b: u32) -> u32 {
    a * b
}

fn handle_sleep_us(cx: &mut ExceptionContext, duration_us: u64) {
    let duration = core::time::Duration::from_micros(duration_us);
    crate::thread::sleep_current_thread(cx, duration);
}

fn handle_yield_exec(cx: &mut ExceptionContext) {
    crate::thread::run_scheduler(cx);
}

fn handle_thread_exit(cx: &mut ExceptionContext) {
    crate::thread::exit_current_thread(cx);
}

fn handle_thread_join(cx: &mut ExceptionContext, tid: u64) {
    crate::thread::join_thread(cx, tid);
}

fn handle_puts(_cx: &mut ExceptionContext, str_ptr: *const u8, length: usize) {
    if str_ptr.is_null() {
        return;
    }

    // We have to trust the user process... If a fault happens, it will be delivered to it anyway
    let slice = unsafe { core::slice::from_raw_parts(str_ptr, length) };
    if let Ok(string) = core::str::from_utf8(slice) {
        // TODO(javier-varez): Of course this needs to be redirected to stdout instead of using the klog system...

        log_info!("Message from userspace pid {:?}: {}", current_pid(), string);
    }
}

fn handle_wait_pid(cx: &mut ExceptionContext, pid: u64) -> u64 {
    // Validate pid
    let pid = match process::validate_pid(pid) {
        None => {
            return 0xFFFF;
        }
        Some(val) => val,
    };

    // TODO(javier-varez): Clean this lock mess. This is just used to ensure we don't get switched out
    static spinlock: SpinLock<()> = SpinLock::new(());
    let _lock = spinlock.lock();

    let exit_code = process::do_with_process(&pid, |process| process.exit_code());
    match exit_code {
        Some(val) => val,
        None => {
            thread::wait_for_pid_in_current_thread(cx, pid);
            // Do not use retval here.
            cx.gpr[0]
        }
    }
}

fn handle_exit(cx: &mut ExceptionContext, exit_code: u64) {
    // This can only be called from a process. Calling it from the kernel itself causes a panic
    process::kill_current_process(cx, exit_code).unwrap();
}
