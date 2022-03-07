use crate::{arch::exceptions::ExceptionContext, drivers::wdt, println};

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
        $syscall_hdlr_name();
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty)
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        $syscall_hdlr_name(arg0);
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty)
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        $syscall_hdlr_name(arg0, arg1);
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty, $arg2_ty: ty)
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        let arg2 = $context.gpr[2] as $arg2_ty;
        $syscall_hdlr_name(arg0, arg1, arg2);
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
        $syscall_hdlr_name(arg0, arg1, arg2, arg3);
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        () -> $ret_ty: ty
    ) => {
        let result = $syscall_hdlr_name();
        $context.gpr[0] = result as u64;
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty) -> $ret_ty: ty
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let result = $syscall_hdlr_name(arg0);
        $context.gpr[0] = result as u64;
    };
    (
        $context: expr,
        $syscall_hdlr_name: ident,
        ($arg0_ty: ty, $arg1_ty: ty) -> $ret_ty: ty
    ) => {
        let arg0 = $context.gpr[0] as $arg0_ty;
        let arg1 = $context.gpr[1] as $arg1_ty;
        let result = $syscall_hdlr_name(arg0, arg1);
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
        let result = $syscall_hdlr_name(arg0, arg1, arg2);
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
        let result = $syscall_hdlr_name(arg0, arg1, arg2, arg3);
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
                    panic!("BUG: Received unknown syscall from user process: {}", id);
                }
            };
        }

    };
}

define_syscalls!(
    [0, Noop, noop, handle_noop, ()],
    [1, Reboot, reboot, handle_reboot, ()],
    [0x8000, Multiply, multiply, handle_multiply, (u32, u32) -> u32],
);

pub enum Error {
    UnknownSyscall(u32),
}

fn handle_noop() {
    println!("Syscall Noop");
}

fn handle_reboot() {
    println!("Syscall Reboot - Rebooting computer");
    wdt::service();

    // We hang here never servicing the WDT again, causing a reboot
    loop {
        cortex_a::asm::wfi();
    }
}

fn handle_multiply(a: u32, b: u32) -> u32 {
    println!("Syscall Multiplication");
    a * b
}
