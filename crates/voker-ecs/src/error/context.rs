use alloc::string::String;
use alloc::string::ToString;
use core::fmt::{Debug, Display};

use voker_utils::debug::DebugName;

use crate::tick::Tick;

// -----------------------------------------------------------------------------
// ErrorContext

/// Context for a [`GameError`] to aid in debugging.
///
/// [`GameError`]: struct@super::GameError
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ErrorContext {
    /// Failure raised while running a system.
    System { name: DebugName, last_run: Tick },
    /// Failure raised while running an observer callback.
    Observer { name: DebugName, last_run: Tick },
    /// Failure raised while applying a command.
    Command { name: DebugName },
}

impl Display for ErrorContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::System { name, .. } => write!(f, "System `{name}` failed"),
            Self::Observer { name, .. } => write!(f, "Observer `{name}` failed"),
            Self::Command { name, .. } => write!(f, "Command `{name}` failed"),
        }
    }
}

impl ErrorContext {
    /// The name of the ECS construct that failed.
    ///
    /// For systems, this is the system name.
    /// For commands, this is usually the call-site location string.
    /// For custom contexts, this returns the raw custom message.
    pub fn name(&self) -> String {
        match self {
            Self::System { name, .. } => name.to_string(),
            Self::Observer { name, .. } => name.to_string(),
            Self::Command { name, .. } => name.to_string(),
        }
    }

    /// A string representation of the kind of ECS construct that failed.
    ///
    /// This helper is intended for logging and telemetry labels.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::System { .. } => "system",
            Self::Command { .. } => "command",
            Self::Observer { .. } => "observer",
        }
    }
}
