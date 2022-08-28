use crate::print;

pub trait Logger: crate::drivers::Device + print::Print {}
