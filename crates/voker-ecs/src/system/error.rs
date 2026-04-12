use alloc::borrow::Cow;
use core::fmt::Debug;

use thiserror::Error;

use crate::error::{GameError, Severity};
use crate::system::SystemId;
use crate::utils::DebugName;

// -----------------------------------------------------------------------------
// SystemParamError

#[derive(Clone, Debug, Error, GameError)]
#[game_error(severity = self.severity)]
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

    pub fn with_system<S>(self) -> Self {
        Self {
            system: DebugName::type_name::<S>(),
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
}

// -----------------------------------------------------------------------------
// UninitSystemError

#[derive(Clone, Debug, Error, GameError)]
#[game_error(severity = "warning")]
#[error("Attempt to run an uninitialized system `{system_id}`.")]
pub struct UninitializedSystemError {
    pub system_id: SystemId,
}

impl UninitializedSystemError {
    #[cold]
    pub fn new<S: 'static>() -> Self {
        Self {
            system_id: SystemId::of::<S>(),
        }
    }
}

// -----------------------------------------------------------------------------
// SystemParamError

#[derive(Clone, Debug, Error, GameError)]
#[game_error(severity = "warning")]
#[error("Attempt to run an unregistered system `{system_id}`.")]
pub struct UnregisteredSystemError {
    pub system_id: SystemId,
}

impl UnregisteredSystemError {
    #[cold]
    pub fn new<S: 'static>() -> Self {
        Self {
            system_id: SystemId::of::<S>(),
        }
    }
}

// -----------------------------------------------------------------------------
// SystemError

#[derive(Debug, Error)]
pub enum SystemError {
    #[error("Sytem runtime error: {0}")]
    Runtime(GameError),
    #[error("Sytem param error: {0}")]
    Param(SystemParamError),
    #[error("Unregistered system: {0}")]
    Unregistered(UnregisteredSystemError),
    #[error("Uninitialized system: {0}")]
    Uninitialized(UninitializedSystemError),
}

impl From<GameError> for SystemError {
    fn from(value: GameError) -> Self {
        if value.is::<Self>() {
            *value.downcast::<Self>().unwrap()
        } else if value.is::<SystemParamError>() {
            SystemError::Param(*value.downcast::<SystemParamError>().unwrap())
        } else if value.is::<UninitializedSystemError>() {
            SystemError::Uninitialized(*value.downcast::<UninitializedSystemError>().unwrap())
        } else if value.is::<UnregisteredSystemError>() {
            SystemError::Unregistered(*value.downcast::<UnregisteredSystemError>().unwrap())
        } else {
            SystemError::Runtime(value)
        }
    }
}

impl From<SystemError> for GameError {
    fn from(value: SystemError) -> Self {
        match value {
            SystemError::Runtime(e) => e,
            SystemError::Param(e) => e.into(),
            SystemError::Unregistered(e) => e.into(),
            SystemError::Uninitialized(e) => e.into(),
        }
    }
}

impl From<SystemParamError> for SystemError {
    fn from(value: SystemParamError) -> Self {
        Self::Param(value)
    }
}

impl From<UninitializedSystemError> for SystemError {
    fn from(value: UninitializedSystemError) -> Self {
        Self::Uninitialized(value)
    }
}

impl From<UnregisteredSystemError> for SystemError {
    fn from(value: UnregisteredSystemError) -> Self {
        Self::Unregistered(value)
    }
}
