#[repr(transparent)]
pub struct Inaccessible<T>(T);

mod sys_impl_apl_ehid4 {
    tock_registers::register_bitfields! { u64,
        pub SYS_IMPL_APL_EHID4 [
            DISABLE_DC_MVA OFFSET(11) NUMBITS(1) [],
            DISABLE_DC_SW_L2_OPS OFFSET(44) NUMBITS(1) [],
            STNT_COUNTER_THRESHOLD OFFSET(40) NUMBITS(2) [],
            ENABLE_LFSR_STALL_LOAD_PIPE_2_ISSUE OFFSET(49) NUMBITS(1) [],
            ENABLE_LFSR_STALL_STQ_REPLAY OFFSET(53) NUMBITS(1) [],
        ]
    }

    crate::define_register!(
        SYS_IMPL_APL_EHID4,
        SYS_IMPL_APL_EHID4::Register,
        3,
        0,
        15,
        4,
        1
    );
}

pub use sys_impl_apl_ehid4::SYS_IMPL_APL_EHID4;

mod sys_impl_apl_hid4 {
    tock_registers::register_bitfields! { u64,
        pub SYS_IMPL_APL_HID4 [
            DISABLE_DC_MVA OFFSET(11) NUMBITS(1) [],
            DISABLE_DC_SW_L2_OPS OFFSET(44) NUMBITS(1) [],
            STNT_COUNTER_THRESHOLD OFFSET(40) NUMBITS(2) [],
            ENABLE_LFSR_STALL_LOAD_PIPE_2_ISSUE OFFSET(49) NUMBITS(1) [],
            ENABLE_LFSR_STALL_STQ_REPLAY OFFSET(53) NUMBITS(1) [],
        ]
    }

    crate::define_register!(
        SYS_IMPL_APL_HID4,
        SYS_IMPL_APL_HID4::Register,
        3,
        0,
        15,
        4,
        0
    );
}

pub use sys_impl_apl_hid4::SYS_IMPL_APL_HID4;

mod sys_impl_apl_hid5 {
    tock_registers::register_bitfields! { u64,
        pub SYS_IMPL_APL_HID5 [
            DISABLE_FILL_2C_MERGE OFFSET(61) NUMBITS(1) [],
        ]
    }

    crate::define_register!(
        SYS_IMPL_APL_HID5,
        SYS_IMPL_APL_HID5::Register,
        3,
        0,
        15,
        5,
        0
    );
}

pub use sys_impl_apl_hid5::SYS_IMPL_APL_HID5;

mod sys_impl_apl_ehid9 {
    tock_registers::register_bitfields! { u64,
        pub SYS_IMPL_APL_EHID9 [
            DEV_THROTTLE_2_ENABLE OFFSET(5) NUMBITS(1) [],
        ]
    }

    crate::define_register!(
        SYS_IMPL_APL_EHID9,
        SYS_IMPL_APL_EHID9::Register,
        3,
        0,
        15,
        9,
        1
    );
}

pub use sys_impl_apl_ehid9::SYS_IMPL_APL_EHID9;

mod sys_impl_apl_ehid10 {
    tock_registers::register_bitfields! { u64,
        pub SYS_IMPL_APL_EHID10 [
            FORCE_WAIT_STATE_DRAIN_UC OFFSET(32) NUMBITS(1) [],
            DISABLE_ZVA_TEMPORAL_TSO OFFSET(49) NUMBITS(1) [],
        ]
    }

    crate::define_register!(
        SYS_IMPL_APL_EHID10,
        SYS_IMPL_APL_EHID10::Register,
        3,
        0,
        15,
        10,
        1
    );
}

pub use sys_impl_apl_ehid10::SYS_IMPL_APL_EHID10;

mod sys_impl_apl_ehid20 {
    tock_registers::register_bitfields! { u64,
        pub SYS_IMPL_APL_EHID20 [
            TRAP_SMC OFFSET(8) NUMBITS(1) [],
            FORCE_NONSPEC_IF_OLDEST_REDIR_VALID_AND_OLDER OFFSET(15) NUMBITS(1) [],
            FORCE_NONSPEC_IF_SPEC_FLUSH_POINTER_NE_BLK_RTR_POINTER OFFSET(16) NUMBITS(1) [],
            FORCE_NONSPEC_TARGETED_TIMER_SEL OFFSET(21) NUMBITS(2) [],
        ]
    }

    crate::define_register!(
        SYS_IMPL_APL_EHID20,
        SYS_IMPL_APL_EHID20::Register,
        3,
        0,
        15,
        1,
        2
    );
}

pub use sys_impl_apl_ehid20::SYS_IMPL_APL_EHID20;

mod s3_4_c15_c5_0 {
    crate::define_register!(S3_4_C15_C5_0, (), 3, 4, 15, 5, 0);
}

pub use s3_4_c15_c5_0::S3_4_C15_C5_0;

mod sys_impl_apl_amx_ctl_el1 {
    crate::define_register!(SYS_IMPL_APL_AMX_CTL_EL1, (), 3, 4, 15, 1, 4);
}

pub use sys_impl_apl_amx_ctl_el1::SYS_IMPL_APL_AMX_CTL_EL1;

mod sys_impl_apl_amx_ctl_el2 {
    crate::define_register!(SYS_IMPL_APL_AMX_CTL_EL2, (), 3, 4, 15, 4, 7);
}

pub use sys_impl_apl_amx_ctl_el2::SYS_IMPL_APL_AMX_CTL_EL2;

mod sys_impl_apl_amx_ctl_el12 {
    crate::define_register!(SYS_IMPL_APL_AMX_CTL_EL12, (), 3, 4, 15, 4, 6);
}

pub use sys_impl_apl_amx_ctl_el12::SYS_IMPL_APL_AMX_CTL_EL12;

mod s3_4_c15_c10_4 {
    crate::define_register!(S3_4_C15_C10_4, (), 3, 4, 15, 10, 4);
}

pub use s3_4_c15_c10_4::S3_4_C15_C10_4;

mod sys_impl_apl_cyc_ovrd {
    tock_registers::register_bitfields! { u64,
        pub SYS_IMPL_APL_CYC_OVRD [
            FIQ_MODE OFFSET(20) NUMBITS(2) [],
            IRQ_MODE OFFSET(22) NUMBITS(2) [],
            WFI_MODE OFFSET(24) NUMBITS(2) [],
            DISABLE_WFI_RET OFFSET(0) NUMBITS(1) [],
        ]
    }

    crate::define_register!(
        SYS_IMPL_APL_CYC_OVRD,
        SYS_IMPL_APL_CYC_OVRD::Register,
        3,
        5,
        15,
        5,
        0
    );
}

pub use sys_impl_apl_cyc_ovrd::SYS_IMPL_APL_CYC_OVRD;

mod sys_impl_apl_acc_cfg {
    tock_registers::register_bitfields! { u64,
        pub SYS_IMPL_APL_ACC_CFG [
            BP_SLEEP OFFSET(2) NUMBITS(2) [],
        ]
    }

    crate::define_register!(
        SYS_IMPL_APL_ACC_CFG,
        SYS_IMPL_APL_ACC_CFG::Register,
        3,
        5,
        15,
        4,
        0
    );
}

pub use sys_impl_apl_acc_cfg::SYS_IMPL_APL_ACC_CFG;

mod sys_impl_apl_pmcr0 {
    crate::define_register!(SYS_IMPL_APL_PMCR0, (), 3, 1, 15, 0, 0);
}

pub use sys_impl_apl_pmcr0::SYS_IMPL_APL_PMCR0;
