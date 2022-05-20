pub trait Error: core::fmt::Debug {
    fn source(&self) -> Option<&(dyn Error + 'static)>;
}
