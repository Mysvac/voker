use core::error::Error;
use core::fmt::{Debug, Display};

use crate::system::SystemName;

#[derive(Clone)]
pub struct UninitSystemError {
    pub name: SystemName,
}

impl Debug for UninitSystemError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "Uninitialized system {}.", self.name)
    }
}

impl Display for UninitSystemError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "Uninitialized system {}.", self.name)
    }
}

impl Error for UninitSystemError {}
