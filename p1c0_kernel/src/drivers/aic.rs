use super::interfaces::interrupt_controller::{InterruptController, IrqType};
use crate::{
    adt::{self},
    error,
    memory::{self, address::Address, MemoryManager},
    prelude::*,
    sync::spinlock::RwSpinLock,
};

use p1c0_macros::initcall;

use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
    registers::{ReadOnly, ReadWrite},
};

#[derive(Debug)]
pub enum Error {
    NotCompatible,
    ProbeError(memory::Error),
    InvalidIrqNumber,
    InvalidAdtNode,
}

impl From<memory::Error> for Box<dyn error::Error> {
    fn from(error: memory::Error) -> Self {
        Box::new(Error::ProbeError(error))
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

register_bitfields![u32,
    Version [
        Version OFFSET(0) NUMBITS(8) [],
    ],
    Info1 [
        IrqNr OFFSET(0) NUMBITS(16) [],
        LastDie OFFSET(24) NUMBITS(4) [],
    ],
    Info3 [
        MaxIrq OFFSET(0) NUMBITS(16) [],
        MaxDie OFFSET(24) NUMBITS(4) [],
    ],
    Reset [
        Reset OFFSET(0) NUMBITS(1) []
    ],
    Config [
        Enable OFFSET(0) NUMBITS(1) [],
        PreferPCPU OFFSET(28) NUMBITS(1) [],
    ],
    IrqConfig [
        Target OFFSET(0) NUMBITS(4) []
    ],
    Event [
        IrqNr OFFSET(0) NUMBITS(16) [],
        Type OFFSET(16) NUMBITS(8) [
            FIQ = 0,
            HW = 1,
            IPI = 4,
        ],
        Die OFFSET(24) NUMBITS(8) []
    ]
];

const IRQ_CONFIG_OFFSET: usize = 0x2000;
const EVENT_OFFSET: usize = 0xC000;

// This value is currently fixed for m1 pro/max
const MAX_IRQS: usize = 4096;

#[repr(C)]
struct AicGlobalRegs {
    version: ReadOnly<u32, Version::Register>,
    info1: ReadOnly<u32, Info1::Register>,
    info2: ReadOnly<u32>,
    info3: ReadOnly<u32, Info3::Register>,
    reset: ReadWrite<u32, Reset::Register>,
    config: ReadWrite<u32, Config::Register>,
}

#[repr(C)]
struct AicRegs {
    config: [ReadWrite<u32, IrqConfig::Register>; MAX_IRQS],
    sw_set: [ReadWrite<u32>; MAX_IRQS / 32],
    sw_clr: [ReadWrite<u32>; MAX_IRQS / 32],
    mask_set: [ReadWrite<u32>; MAX_IRQS / 32],
    mask_clr: [ReadWrite<u32>; MAX_IRQS / 32],
    hw_state: [ReadWrite<u32>; MAX_IRQS / 32],
}

#[repr(C)]
struct AicEventRegs {
    event: ReadWrite<u32, Event::Register>,
}

pub struct Aic {
    global_regs: &'static mut AicGlobalRegs,
    irq_regs: &'static mut AicRegs,
    event_regs: &'static mut AicEventRegs,
}

impl Aic {
    pub fn probe(dev_path: &[adt::AdtNode]) -> Result<super::DeviceRef, Box<dyn error::Error>> {
        let adt = adt::get_adt().expect("Could not get adt");
        let (aic_pa, size) = adt
            .get_device_addr_from_nodes(dev_path, 0)
            .ok_or_else(|| Box::new(Error::InvalidAdtNode) as Box<dyn error::Error>)?;

        let va = MemoryManager::instance().map_io("aic", aic_pa, size)?;

        let global_regs = unsafe { &mut *(va.as_mut_ptr() as *mut AicGlobalRegs) };
        let irq_regs = unsafe { &mut *(va.offset(IRQ_CONFIG_OFFSET).as_mut_ptr() as *mut AicRegs) };
        let event_regs =
            unsafe { &mut *(va.offset(EVENT_OFFSET).as_mut_ptr() as *mut AicEventRegs) };

        let mut instance = Self {
            global_regs,
            irq_regs,
            event_regs,
        };

        instance.mask_all()?;
        instance.global_regs.config.write(Config::Enable::SET);

        let instance = Arc::new(RwSpinLock::new(super::Dev::InterruptController(Box::new(
            instance,
        ))));
        super::interfaces::interrupt_controller::register_interrupt_controller(instance.clone());

        Ok(instance)
    }

    fn offset_for_irq_number(&self, irq_number: u32) -> Result<(u32, u32), Box<dyn error::Error>> {
        if irq_number >= self.num_interrupts() {
            return Err(Box::new(Error::InvalidIrqNumber));
        }

        let reg_offset = irq_number / 32;
        let bit_offset = irq_number % 32;
        Ok((reg_offset, bit_offset))
    }
}

impl InterruptController for Aic {
    fn num_interrupts(&self) -> u32 {
        self.global_regs.info1.read(Info1::IrqNr)
    }

    fn mask_interrupt(&mut self, irq_number: u32) -> Result<(), Box<dyn error::Error>> {
        let (reg_offset, bit_offset) = self.offset_for_irq_number(irq_number)?;
        self.irq_regs.mask_set[reg_offset as usize].set(1 << bit_offset);
        Ok(())
    }

    fn unmask_interrupt(&mut self, irq_number: u32) -> Result<(), Box<dyn error::Error>> {
        let (reg_offset, bit_offset) = self.offset_for_irq_number(irq_number)?;
        self.irq_regs.mask_clr[reg_offset as usize].set(1 << bit_offset);
        Ok(())
    }

    fn set_interrupt(&mut self, irq_number: u32) -> Result<(), Box<dyn error::Error>> {
        let (reg_offset, bit_offset) = self.offset_for_irq_number(irq_number)?;
        self.irq_regs.sw_set[reg_offset as usize].set(1 << bit_offset);
        Ok(())
    }

    fn clear_interrupt(&mut self, irq_number: u32) -> Result<(), Box<dyn error::Error>> {
        let (reg_offset, bit_offset) = self.offset_for_irq_number(irq_number)?;
        self.irq_regs.sw_clr[reg_offset as usize].set(1 << bit_offset);
        Ok(())
    }

    fn get_current_irq(&mut self) -> Option<(u32, u32, IrqType)> {
        let reg = self.event_regs.event.extract();

        if reg.get() == 0 {
            return None;
        }

        let r#type = match reg.read_as_enum(Event::Type) {
            Some(Event::Type::Value::FIQ) => IrqType::FIQ,
            Some(Event::Type::Value::IPI) => IrqType::IPI,
            Some(Event::Type::Value::HW) => IrqType::HW,
            None => {
                panic!(
                    "Unknown IRQ type read out from register! {}",
                    self.event_regs.event.read(Event::Type)
                );
            }
        };
        let number = reg.read(Event::IrqNr);
        let die = reg.read(Event::Die);
        Some((die, number, r#type))
    }
}

impl super::Device for Aic {}

struct AicDriver {}

impl super::Driver for AicDriver {
    fn probe(&self, dev_path: &[adt::AdtNode]) -> super::Result<super::DeviceRef> {
        log_error!("Probing aic driver");
        let dev = Aic::probe(dev_path).map_err(|e| super::Error::DeviceSpecificError(e))?;
        Ok(dev)
    }
}

#[initcall(priority = 0)]
fn register_aic_driver() {
    super::register_driver("aic,2", Box::new(AicDriver {})).unwrap();
}
