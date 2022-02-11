use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
    registers::{ReadOnly, ReadWrite},
};

use crate::{
    adt::get_adt,
    memory::{self, address::Address, MemoryManager},
};

#[derive(Debug)]
pub enum Error {
    NotCompatible,
    ProbeError(memory::Error),
    InvalidIrqNumber,
}

impl From<memory::Error> for Error {
    fn from(error: memory::Error) -> Self {
        Self::ProbeError(error)
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
}

#[repr(C)]
struct AicEventRegs {
    event: ReadWrite<u32, Event::Register>,
}

#[derive(Debug)]
pub enum IrqType {
    FIQ,
    HW,
    IPI,
}

pub struct Aic {
    global_regs: &'static mut AicGlobalRegs,
    irq_regs: &'static mut AicRegs,
    event_regs: &'static mut AicEventRegs,
}

impl Aic {
    pub fn probe(device_path: &str) -> Result<Self, Error> {
        let adt = get_adt().expect("Could not get adt");

        let node = adt.find_node(device_path).ok_or(Error::NotCompatible)?;
        if !node.is_compatible("aic,2") {
            return Err(Error::NotCompatible);
        }

        let (aic_pa, size) = adt.get_device_addr(device_path, 0).unwrap();

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

        Ok(instance)
    }

    fn offset_for_irq_number(&self, irq_number: u32) -> Result<(u32, u32), Error> {
        if irq_number >= self.num_interrupts() {
            return Err(Error::InvalidIrqNumber);
        }

        let reg_offset = irq_number / 32;
        let bit_offset = irq_number % 32;
        Ok((reg_offset, bit_offset))
    }

    pub fn mask_interrupt(&mut self, irq_number: u32) -> Result<(), Error> {
        let (reg_offset, bit_offset) = self.offset_for_irq_number(irq_number)?;
        self.irq_regs.mask_set[reg_offset as usize].set(1 << bit_offset);
        Ok(())
    }

    pub fn unmask_interrupt(&mut self, irq_number: u32) -> Result<(), Error> {
        let (reg_offset, bit_offset) = self.offset_for_irq_number(irq_number)?;
        self.irq_regs.mask_clr[reg_offset as usize].set(1 << bit_offset);
        Ok(())
    }

    pub fn set_interrupt(&mut self, irq_number: u32) -> Result<(), Error> {
        let (reg_offset, bit_offset) = self.offset_for_irq_number(irq_number)?;
        self.irq_regs.sw_set[reg_offset as usize].set(1 << bit_offset);
        Ok(())
    }

    pub fn clear_interrupt(&mut self, irq_number: u32) -> Result<(), Error> {
        let (reg_offset, bit_offset) = self.offset_for_irq_number(irq_number)?;
        self.irq_regs.sw_clr[reg_offset as usize].set(1 << bit_offset);
        Ok(())
    }

    pub fn num_interrupts(&self) -> u32 {
        self.global_regs.info1.read(Info1::IrqNr)
    }

    pub fn mask_all(&mut self) -> Result<(), Error> {
        for i in 0..self.num_interrupts() {
            self.mask_interrupt(i)?;
        }
        Ok(())
    }

    pub fn get_current_irq_number(&mut self) -> u32 {
        self.event_regs.event.read(Event::IrqNr)
    }

    pub fn get_current_irq_type(&mut self) -> IrqType {
        match self.event_regs.event.read_as_enum(Event::Type) {
            Some(Event::Type::Value::FIQ) => IrqType::FIQ,
            Some(Event::Type::Value::IPI) => IrqType::IPI,
            Some(Event::Type::Value::HW) => IrqType::HW,
            None => {
                panic!(
                    "Unknow IRQ type read out from register! {}",
                    self.event_regs.event.read(Event::Type)
                );
            }
        }
    }
}

pub static mut AIC: Option<Aic> = None;
