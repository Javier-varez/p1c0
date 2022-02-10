use tock_registers::{
    interfaces::{ReadWriteable, Readable},
    register_bitfields,
    registers::ReadWrite,
};

use crate::{
    adt,
    memory::{self, address::Address, MemoryManager},
};

register_bitfields! {u32,
    Pin [
        DATA OFFSET(0) NUMBITS(1) [ON=1, OFF=0],
        MODE OFFSET(1) NUMBITS(3) [
            OUT = 1,
            IN_IRQ_HI = 2,
            IN_IRQ_LO = 3,
            IN_IRQ_UP = 4,
            IN_IRQ_DOWN = 5,
            IN_IRQ_ANY = 6,
            IN_IRQ_OFF = 7,
        ],
        PERIPH OFFSET(5) NUMBITS(2) [],
        PULL OFFSET(7) NUMBITS(2) [
            OFF = 0,
            DOWN = 1,
            UP_STRONG = 2,
            UP = 3,
        ],
        INPUT OFFSET(9) NUMBITS(1) [
            DISABLE = 0,
            ENABLE = 1,
        ],
        DRIVE_STRENGTH0 OFFSET(10) NUMBITS(2) [],
        SCHMITT OFFSET(15) NUMBITS(1) [],
        GRP OFFSET(16) NUMBITS(3) [],
        LOCK OFFSET(21) NUMBITS(1) [],
        DRIVE_STRENGTH1 OFFSET(22) NUMBITS(2) []
    ]
}

#[derive(Debug)]
pub enum Error {
    AdtNotAvailable(adt::Error),
    NodeNotCompatible,
    MissingAdtProperty(&'static str),
    MmioError(memory::Error),
    InvalidPin,
    InvalidDirection,
}

impl From<memory::Error> for Error {
    fn from(error: memory::Error) -> Self {
        Error::MmioError(error)
    }
}

pub enum PinState {
    Low,
    High,
}

impl From<PinState> for u32 {
    fn from(state: PinState) -> Self {
        match state {
            PinState::Low => 0,
            PinState::High => 1,
        }
    }
}

pub enum PinDirection {
    Input,
    Output(PinState),
}

pub struct GpioBank {
    regs: &'static mut [ReadWrite<u32, Pin::Register>],
}

impl GpioBank {
    pub fn new(gpio_bank: &str) -> Result<Self, Error> {
        let adt = adt::get_adt().map_err(Error::AdtNotAvailable)?;

        let node = adt.find_node(gpio_bank);

        match node
            .as_ref()
            .and_then(|node| node.find_property("compatible"))
            .and_then(|property| property.str_value().ok())
        {
            Some(compatible) if compatible == "gpio,t6000" => {}
            _ => {
                return Err(Error::NodeNotCompatible);
            }
        }

        let node = node.unwrap();

        let (pa, size) = adt
            .get_device_addr(gpio_bank, 0)
            .ok_or(Error::MissingAdtProperty("reg"))?;

        let va = MemoryManager::instance().map_io(gpio_bank, pa, size)?;

        if let Some(num_pins) = node
            .find_property("#gpio-pins")
            .and_then(|prop| prop.u32_value().ok())
        {
            let regs = unsafe {
                core::slice::from_raw_parts_mut(va.as_mut_ptr() as *mut _, num_pins as usize)
            };

            Ok(Self { regs })
        } else {
            crate::println!(
                "Cannot instantiate gpio {}. Missing property #gpio-pins.",
                gpio_bank
            );
            Err(Error::MissingAdtProperty("#gpio-pins"))
        }
    }

    fn get_mut_pin(
        &mut self,
        pin_index: usize,
    ) -> Result<&mut ReadWrite<u32, Pin::Register>, Error> {
        if pin_index >= self.regs.len() {
            return Err(Error::InvalidPin);
        }
        Ok(&mut self.regs[pin_index])
    }

    fn get_pin(&self, pin_index: usize) -> Result<&ReadWrite<u32, Pin::Register>, Error> {
        if pin_index >= self.regs.len() {
            return Err(Error::InvalidPin);
        }
        Ok(&self.regs[pin_index])
    }

    // TODO(javier-varez): Implement some pin ownership mechanism and avoid these bare methods

    pub fn get_pin_value(&self, pin_index: usize) -> Result<PinState, Error> {
        let pin = self.get_pin(pin_index)?;

        if pin.read(Pin::DATA) == 0 {
            Ok(PinState::Low)
        } else {
            Ok(PinState::High)
        }
    }

    pub fn get_pin_direction(&self, pin_index: usize) -> Result<PinDirection, Error> {
        let pin = self.get_pin(pin_index)?;

        let direction = match pin.read_as_enum(Pin::MODE).unwrap() {
            Pin::MODE::Value::OUT => {
                let state = self.get_pin_value(pin_index)?;
                PinDirection::Output(state)
            }
            _ => PinDirection::Input,
        };

        Ok(direction)
    }

    pub fn set_pin_direction(
        &mut self,
        pin_index: usize,
        direction: PinDirection,
    ) -> Result<(), Error> {
        let pin = self.get_mut_pin(pin_index)?;

        match direction {
            PinDirection::Input => pin.modify(Pin::MODE::IN_IRQ_OFF),
            PinDirection::Output(initial_state) => {
                pin.modify(Pin::MODE::OUT + Pin::DATA.val(initial_state.into()))
            }
        }

        Ok(())
    }

    pub fn set_pin_state(&mut self, pin_index: usize, state: PinState) -> Result<(), Error> {
        // Check that the pin is an output
        match self.get_pin_direction(pin_index)? {
            PinDirection::Output(_) => {}
            _ => return Err(Error::InvalidDirection),
        };

        let pin = self.get_mut_pin(pin_index)?;
        pin.modify(Pin::DATA.val(state.into()));
        Ok(())
    }
}
