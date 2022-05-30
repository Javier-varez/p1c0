use crate::print;

pub trait Logger {
    fn write_u8(&mut self, c: u8) -> Result<(), print::Error>;
}
