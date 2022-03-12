use tock_registers::{
    interfaces::{ReadWriteable, Readable},
    register_bitfields,
    registers::ReadWrite,
};

use crate::{
    adt, log_error,
    memory::{self, address::Address, MemoryManager},
    sync::spinlock::SpinLock,
};

register_bitfields! {u32,
    PinReg [
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
    PinNotAvailable,
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

pub mod mode {
    pub struct Input {}

    pub struct Output {}
}

const MAX_PINS: usize = 256;

pub struct GpioBank {
    regs: *mut ReadWrite<u32, PinReg::Register>,
    num_pins: usize,
    taken: SpinLock<[bool; MAX_PINS]>,
}

pub struct Pin<'a, MODE> {
    bank: &'a GpioBank,
    reg: *mut ReadWrite<u32, PinReg::Register>,
    index: usize,
    _pd: core::marker::PhantomData<MODE>,
}

impl<'a, MODE> Pin<'a, MODE> {
    fn reg(&self) -> &'static ReadWrite<u32, PinReg::Register> {
        unsafe { &mut *self.reg }
    }

    pub fn get_pin_state(&self) -> PinState {
        if self.reg().read(PinReg::DATA) == 0 {
            PinState::Low
        } else {
            PinState::High
        }
    }
}

impl<'a> Pin<'a, mode::Output> {
    pub fn into_input(mut self) -> Pin<'a, mode::Input> {
        self.reg().modify(PinReg::MODE::IN_IRQ_OFF);

        let new_pin = Pin {
            bank: self.bank,
            reg: self.reg,
            index: self.index,
            _pd: core::marker::PhantomData {},
        };

        // Invalidate the pin so that drop does not free it
        self.index = usize::MAX;
        new_pin
    }

    pub fn set_pin_state(&mut self, state: PinState) {
        self.reg().modify(PinReg::DATA.val(state.into()));
    }
}

impl<'a> Pin<'a, mode::Input> {
    pub fn into_output(mut self, initial_state: PinState) -> Pin<'a, mode::Output> {
        self.reg()
            .modify(PinReg::MODE::OUT + PinReg::DATA.val(initial_state.into()));

        let new_pin = Pin {
            bank: self.bank,
            reg: self.reg,
            index: self.index,
            _pd: core::marker::PhantomData {},
        };

        // Invalidate the pin so that drop does not free it
        self.index = usize::MAX;

        new_pin
    }
}

impl<'a, MODE> Drop for Pin<'a, MODE> {
    fn drop(&mut self) {
        if self.index != usize::MAX {
            self.bank.release_pin(self.index);
        }
    }
}

impl GpioBank {
    /// Constucts a new GpioBank peripheral from the given adt node reference.
    ///
    /// # Safety
    /// The gpio_bank must not already be in use by any other piece of code.
    pub unsafe fn new(gpio_bank: &str) -> Result<Self, Error> {
        let adt = adt::get_adt().map_err(Error::AdtNotAvailable)?;

        let node = adt.find_node(gpio_bank).ok_or(Error::NodeNotCompatible)?;
        if !node.is_compatible("gpio,t6000") {
            return Err(Error::NodeNotCompatible);
        }

        let (pa, size) = adt
            .get_device_addr(gpio_bank, 0)
            .ok_or(Error::MissingAdtProperty("reg"))?;

        let va = MemoryManager::instance().map_io(gpio_bank, pa, size)?;

        if let Some(num_pins) = node
            .find_property("#gpio-pins")
            .and_then(|prop| prop.u32_value().ok())
        {
            Ok(Self {
                regs: va.as_mut_ptr() as *mut _,
                num_pins: num_pins as usize,
                taken: SpinLock::new([false; MAX_PINS]),
            })
        } else {
            log_error!(
                "Cannot instantiate gpio {}. Missing property #gpio-pins.",
                gpio_bank
            );
            Err(Error::MissingAdtProperty("#gpio-pins"))
        }
    }

    fn try_take_pin(&self, index: usize) -> Result<(), Error> {
        if index >= self.num_pins {
            return Err(Error::InvalidPin);
        }

        let mut taken = self.taken.lock();
        if taken[index] {
            return Err(Error::PinNotAvailable);
        }
        taken[index] = true;

        Ok(())
    }

    fn release_pin(&self, index: usize) {
        let mut taken = self.taken.lock();
        assert!(taken[index]);
        taken[index] = false;
    }

    pub fn request_as_input(&self, index: usize) -> Result<Pin<'_, mode::Input>, Error> {
        self.try_take_pin(index)?;

        let reg = unsafe { &mut *self.regs.add(index) };
        reg.modify(PinReg::MODE::IN_IRQ_OFF);

        Ok(Pin {
            bank: self,
            reg,
            index,
            _pd: core::marker::PhantomData {},
        })
    }

    pub fn request_as_output(
        &self,
        index: usize,
        initial_state: PinState,
    ) -> Result<Pin<'_, mode::Output>, Error> {
        self.try_take_pin(index)?;

        let reg = unsafe { &mut *self.regs.add(index) };
        reg.modify(PinReg::MODE::OUT + PinReg::DATA.val(initial_state.into()));

        Ok(Pin {
            bank: self,
            reg,
            index,
            _pd: core::marker::PhantomData {},
        })
    }
}
