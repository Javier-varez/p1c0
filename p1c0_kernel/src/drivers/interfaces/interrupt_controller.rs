use crate::sync::spinlock::RwSpinLock;
use crate::{error, prelude::*};

type Result<T> = core::result::Result<T, Box<dyn error::Error>>;

#[derive(Debug)]
pub enum IrqType {
    FIQ,
    HW,
    IPI,
}

pub trait InterruptController {
    fn num_interrupts(&self) -> u32;
    fn mask_interrupt(&mut self, irq_number: u32) -> Result<()>;
    fn unmask_interrupt(&mut self, irq_number: u32) -> Result<()>;
    fn set_interrupt(&mut self, irq_number: u32) -> Result<()>;
    fn clear_interrupt(&mut self, irq_number: u32) -> Result<()>;
    fn get_current_irq(&mut self) -> Option<(u32, u32, IrqType)>;

    fn mask_all(&mut self) -> Result<()> {
        for i in 0..self.num_interrupts() {
            self.mask_interrupt(i)?;
        }
        Ok(())
    }
}

// Assume just 1 interrupt controller for now. This might have to change in the future
static IRQ_CONTROLLER: RwSpinLock<Option<crate::drivers::DeviceRef>> = RwSpinLock::new(None);

pub fn register_interrupt_controller(irq_controller: crate::drivers::DeviceRef) {
    match &*irq_controller.lock_read() {
        crate::drivers::Dev::InterruptController(_) => {}
        _ => {
            panic!("Device must be an interrupt controller");
        }
    }
    IRQ_CONTROLLER.lock_write().replace(irq_controller);
}

pub fn may_do_with_irq_controller(
    mut callable: impl FnMut(&mut Box<dyn InterruptController>),
) -> bool {
    let irq_ctrlrer_guard = IRQ_CONTROLLER.lock_read();
    if let Some(irq_controller) = irq_ctrlrer_guard.as_ref() {
        let mut irq_controller = irq_controller.lock_write();
        match &mut *irq_controller {
            crate::drivers::Dev::InterruptController(irq_controller) => {
                callable(irq_controller);
                return true;
            }
            _ => unreachable!(),
        };
    }

    false
}
