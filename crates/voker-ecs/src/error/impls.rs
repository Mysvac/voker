use core::convert::Infallible;

use crate::error::{GameError, Severity};

impl From<Infallible> for GameError {
    fn from(error: Infallible) -> Self {
        GameError::new(Severity::Panic, error)
    }
}
