use core::fmt::Debug;

use thiserror::Error;

use crate::utils::DebugName;

#[derive(Clone, Copy, Debug, Error)]
#[error("Uninitialized system {name}.")]
pub struct UninitSystemError {
    pub name: DebugName,
}

#[derive(Clone, Copy, Debug, Error)]
#[error("Uninitialized resource {name}.")]
pub struct UninitResourceError {
    pub name: DebugName,
}
