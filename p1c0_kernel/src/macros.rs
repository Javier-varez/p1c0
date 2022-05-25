#[macro_export]
macro_rules! named_register_string {
    ($op0: literal,
    $op1: literal, $crn: literal, $crm: literal, $op2: literal) => {
        concat!("s", $op0, "_", $op1, "_c", $crn, "_c", $crm, "_", $op2)
    };
}

#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! define_register {
    ($name: ident, $type: ty, $op0: literal,
    $op1: literal, $crn: literal, $crm: literal, $op2: literal) => {
        #[allow(non_snake_case)]
        pub struct Reg;

        impl tock_registers::interfaces::Readable for Reg {
            type T = u64;
            type R = $type;

            fn get(&self) -> Self::T {
                #[cfg(target_arch = "aarch64")]
                unsafe {
                    let mut value : u64;
                    core::arch::asm!(concat!("mrs {}, ", crate::named_register_string!($op0, $op1, $crn, $crm, $op2)), out(reg) value);
                    value
                }

                #[cfg(not(target_arch = "aarch64"))]
                0
            }
        }

        impl tock_registers::interfaces::Writeable for Reg {
            type T = u64;
            type R = $type;

            fn set(&self, value: Self::T) {
                #[cfg(target_arch = "aarch64")]
                unsafe {
                    core::arch::asm!(concat!("msr ", crate::named_register_string!($op0, $op1, $crn, $crm, $op2), ", {}"), in(reg) value);
                }

                #[cfg(not(target_arch = "aarch64"))]
                let _ = value;
            }
        }

        pub const $name: Reg = Reg {};
    };
}
