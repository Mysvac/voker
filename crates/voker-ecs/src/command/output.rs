use crate::error::{GameError, Severity};

// -----------------------------------------------------------------------------
// CommandOutput

pub trait CommandOutput: Sized {
    fn to_err(self) -> Option<GameError>;

    #[inline]
    fn with_severity(self, severity: Severity) -> Option<GameError> {
        self.to_err().map(|e| e.with_severity(severity))
    }

    #[inline]
    fn merge_severity(self, severity: Severity) -> Option<GameError> {
        self.to_err().map(|e| e.merge_severity(severity))
    }

    #[inline]
    fn map_severity(self, f: impl FnOnce(Severity) -> Severity) -> Option<GameError> {
        self.to_err().map(|e| e.map_severity(f))
    }
}

// -----------------------------------------------------------------------------
// Basic

impl CommandOutput for () {
    #[inline(always)]
    fn to_err(self) -> Option<GameError> {
        None
    }

    #[inline(always)]
    fn with_severity(self, _: Severity) -> Option<GameError> {
        None
    }

    #[inline(always)]
    fn merge_severity(self, _: Severity) -> Option<GameError> {
        None
    }

    #[inline(always)]
    fn map_severity(self, _: impl FnOnce(Severity) -> Severity) -> Option<GameError> {
        None
    }
}

impl CommandOutput for GameError {
    #[inline(always)]
    fn to_err(self) -> Option<GameError> {
        Some(self)
    }

    #[inline]
    fn with_severity(self, severity: Severity) -> Option<GameError> {
        Some(GameError::with_severity(self, severity))
    }

    #[inline]
    fn merge_severity(self, severity: Severity) -> Option<GameError> {
        Some(GameError::merge_severity(self, severity))
    }

    #[inline]
    fn map_severity(self, f: impl FnOnce(Severity) -> Severity) -> Option<GameError> {
        Some(GameError::map_severity(self, f))
    }
}

impl<T: CommandOutput> CommandOutput for Option<T> {
    #[inline(always)]
    fn to_err(self) -> Option<GameError> {
        self.and_then(CommandOutput::to_err)
    }
}

impl<T: CommandOutput, E: CommandOutput> CommandOutput for Result<T, E> {
    #[inline(always)]
    fn to_err(self) -> Option<GameError> {
        match self {
            Ok(x) => x.to_err(),
            Err(y) => y.to_err(),
        }
    }
}

// -----------------------------------------------------------------------------
// Entity

use crate::entity::{DespawnError, FetchError, MoveError, SpawnError};

impl CommandOutput for MoveError {
    fn to_err(self) -> Option<GameError> {
        Some(GameError::from(self).with_severity(Severity::Error))
    }
}

impl CommandOutput for SpawnError {
    fn to_err(self) -> Option<GameError> {
        Some(GameError::from(self).with_severity(Severity::Warning))
    }
}

impl CommandOutput for FetchError {
    fn to_err(self) -> Option<GameError> {
        Some(GameError::from(self).with_severity(Severity::Info))
    }
}

impl CommandOutput for DespawnError {
    fn to_err(self) -> Option<GameError> {
        Some(GameError::from(self).with_severity(Severity::Info))
    }
}

// -----------------------------------------------------------------------------
// System

use crate::system::{SystemParamError, UninitializedSystemError, UnregisteredSystemError};

impl CommandOutput for UninitializedSystemError {
    fn to_err(self) -> Option<GameError> {
        Some(GameError::from(self).with_severity(Severity::Warning))
    }
}

impl CommandOutput for UnregisteredSystemError {
    fn to_err(self) -> Option<GameError> {
        Some(GameError::from(self).with_severity(Severity::Warning))
    }
}

impl CommandOutput for SystemParamError {
    fn to_err(self) -> Option<GameError> {
        Some(self.into_game_error())
    }
}
