use crate::{prelude::*, registers::*};

use cortex_a::registers::*;
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

#[derive(Debug)]
enum PartNumbers {
    T8103Icestorm = 0x22,
    T8103Firestorm = 0x23,
    T6000Icestorm = 0x24,
    T6000Firestorm = 0x25,
    T6001Icestorm = 0x28,
    T6001Firestorm = 0x29,
    T8112Blizzard = 0x32,
    T8112Avalanche = 0x33,
}

impl TryFrom<u64> for PartNumbers {
    type Error = ();
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0x22 => Ok(PartNumbers::T8103Icestorm),
            0x23 => Ok(PartNumbers::T8103Firestorm),
            0x24 => Ok(PartNumbers::T6000Icestorm),
            0x25 => Ok(PartNumbers::T6000Firestorm),
            0x28 => Ok(PartNumbers::T6001Icestorm),
            0x29 => Ok(PartNumbers::T6001Firestorm),
            0x32 => Ok(PartNumbers::T8112Blizzard),
            0x33 => Ok(PartNumbers::T8112Avalanche),
            _ => Err(()),
        }
    }
}

fn is_ecore() -> bool {
    let mpidr = MPIDR_EL1.get();
    (mpidr & 1 << 16) == 0
}

fn init_common_icestorm() {
    // "Sibling Merge in LLC can cause UC load to violate ARM Memory Ordering Rules."
    SYS_IMPL_APL_HID5.modify(SYS_IMPL_APL_HID5::DISABLE_FILL_2C_MERGE::SET);
    SYS_IMPL_APL_EHID9.modify(SYS_IMPL_APL_EHID9::DEV_THROTTLE_2_ENABLE::CLEAR);

    // "Prevent store-to-load forwarding for UC memory to avoid barrier ordering
    // violation"
    SYS_IMPL_APL_EHID10.modify(
        SYS_IMPL_APL_EHID10::DISABLE_ZVA_TEMPORAL_TSO::SET
            + SYS_IMPL_APL_EHID10::FORCE_WAIT_STATE_DRAIN_UC::SET,
    );

    // FIXME: do we actually need this?
    SYS_IMPL_APL_EHID20.modify(SYS_IMPL_APL_EHID20::TRAP_SMC::SET);
}

fn init_m1_icestorm() {
    init_common_icestorm();

    SYS_IMPL_APL_EHID20.modify(
        SYS_IMPL_APL_EHID20::FORCE_NONSPEC_IF_OLDEST_REDIR_VALID_AND_OLDER::SET
            + SYS_IMPL_APL_EHID20::FORCE_NONSPEC_IF_SPEC_FLUSH_POINTER_NE_BLK_RTR_POINTER::SET,
    );

    SYS_IMPL_APL_EHID20.modify(SYS_IMPL_APL_EHID20::FORCE_NONSPEC_TARGETED_TIMER_SEL.val(3));
}

fn init_common_blizzard() {
    SYS_IMPL_APL_EHID0.modify(SYS_IMPL_APL_EHID0::BLI_UNK32::SET);
}

fn init_m2_blizzard() {
    init_common_blizzard();

    SYS_IMPL_APL_EHID9.modify(SYS_IMPL_APL_EHID9::DEV_THROTTLE_2_LIMIT.val(60));
    SYS_IMPL_APL_EHID9.modify(SYS_IMPL_APL_EHID9::DEV_THROTTLE_2_ENABLE::SET);
}

pub fn init_cpu() {
    OSLAR_EL1.set(0);

    if is_ecore() {
        SYS_IMPL_APL_EHID4.modify(
            SYS_IMPL_APL_EHID4::DISABLE_DC_MVA::SET + SYS_IMPL_APL_EHID4::DISABLE_DC_SW_L2_OPS::SET,
        );
    } else {
        SYS_IMPL_APL_HID4.modify(
            SYS_IMPL_APL_HID4::DISABLE_DC_MVA::SET + SYS_IMPL_APL_HID4::DISABLE_DC_SW_L2_OPS::SET,
        );
    }

    let part: PartNumbers = MIDR_EL1
        .read(MIDR_EL1::PartNum)
        .try_into()
        .expect("Unknown CPU part number");
    let revision = MIDR_EL1.read(MIDR_EL1::Revision);
    log_debug!("Part number: {:?}, Revision: {}", part, revision);

    match part {
        PartNumbers::T6001Firestorm => todo!(),
        PartNumbers::T6001Icestorm => init_m1_icestorm(),
        PartNumbers::T6000Firestorm => todo!(),
        PartNumbers::T6000Icestorm => init_m1_icestorm(),
        PartNumbers::T8103Firestorm => todo!(),
        PartNumbers::T8103Icestorm => init_m1_icestorm(),
        PartNumbers::T8112Avalanche => todo!(),
        PartNumbers::T8112Blizzard => init_m2_blizzard(),
    };

    let core = MPIDR_EL1.get() & 0xff;
    // Unknown, related to SMP?
    S3_4_C15_C5_0.set(core);
    SYS_IMPL_APL_AMX_CTL_EL1.set(0x100);

    S3_4_C15_C10_4.set(0);

    unsafe {
        cortex_a::asm::barrier::isb(cortex_a::asm::barrier::SY);
    }
    SYS_IMPL_APL_CYC_OVRD.modify(
        SYS_IMPL_APL_CYC_OVRD::FIQ_MODE.val(0)
            + SYS_IMPL_APL_CYC_OVRD::IRQ_MODE.val(0)
            + SYS_IMPL_APL_CYC_OVRD::WFI_MODE.val(2),
    );

    SYS_IMPL_APL_ACC_CFG.modify(SYS_IMPL_APL_ACC_CFG::BP_SLEEP.val(3));
}
