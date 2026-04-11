use alloc::borrow::Cow;
use core::fmt::Debug;

use thiserror::Error;

use crate::error::{GameError, Severity};
use crate::system::SystemId;
use crate::utils::DebugName;

// -----------------------------------------------------------------------------
// SystemParamError

#[derive(Clone, Debug, Error)]
#[error("Build system param `{name}` failed in system `{system}`: {info}.")]
pub struct SystemParamError {
    pub name: DebugName,
    pub system: DebugName,
    pub info: Cow<'static, str>,
    pub severity: Severity,
}

impl SystemParamError {
    #[cold]
    pub fn new<Param>() -> Self {
        Self {
            name: DebugName::type_name::<Param>(),
            system: DebugName::anonymous(),
            info: Cow::Borrowed(""),
            severity: Severity::Warning,
        }
    }

    pub fn with_system<System>(self) -> Self {
        Self {
            system: DebugName::type_name::<System>(),
            ..self
        }
    }

    pub fn with_info(self, info: impl Into<Cow<'static, str>>) -> Self {
        Self {
            info: info.into(),
            ..self
        }
    }

    pub fn with_severity(self, severity: Severity) -> Self {
        Self { severity, ..self }
    }

    pub fn into_game_error(self) -> GameError {
        let severity = self.severity;
        GameError::from(self).with_severity(severity)
    }
}

// -----------------------------------------------------------------------------
// UninitSystemError

#[derive(Clone, Debug, Error)]
#[error("Attempt to run an uninitialized system `{system_id}`.")]
pub struct UninitializedSystemError {
    pub system_id: SystemId,
}

impl UninitializedSystemError {
    #[cold]
    pub fn new<T: 'static>() -> Self {
        Self {
            system_id: SystemId::of::<T>(),
        }
    }
}

// -----------------------------------------------------------------------------
// SystemParamError

#[derive(Clone, Debug, Error)]
#[error("Attempt to run an unregistered system `{system_id}`.")]
pub struct UnregisteredSystemError {
    pub system_id: SystemId,
}

impl UnregisteredSystemError {
    #[cold]
    pub fn new<T: 'static>() -> Self {
        Self {
            system_id: SystemId::of::<T>(),
        }
    }
}
