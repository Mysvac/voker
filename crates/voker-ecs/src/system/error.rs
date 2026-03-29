use core::error::Error;
use core::fmt::{Debug, Display};

use crate::system::SystemId;

#[derive(Clone)]
pub struct UninitSystemError {
    pub name: SystemId,
}

impl Debug for UninitSystemError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Uninitialized system {}.", self.name)
    }
}

impl Display for UninitSystemError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Uninitialized system {}.", self.name)
    }
}

impl Error for UninitSystemError {}
