use alloc::string::String;
use alloc::string::ToString;
use core::fmt::{Debug, Display};

use crate::tick::Tick;
use crate::utils::DebugName;

// -----------------------------------------------------------------------------
// ErrorContext

/// Context for a [`GameError`] to aid in debugging.
///
/// [`GameError`]: super::GameError
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ErrorContext {
    System { name: DebugName, last_run: Tick },
    Condition { name: DebugName, last_run: Tick },
    Observer { name: DebugName, last_run: Tick },
    Command { name: DebugName },
}

impl Display for ErrorContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::System { name, .. } => write!(f, "System `{name}` failed"),
            Self::Condition { name, .. } => write!(f, "Condition `{name}` failed"),
            Self::Observer { name, .. } => write!(f, "Observer `{name}` failed"),
            Self::Command { name, .. } => write!(f, "Command `{name}` failed"),
        }
    }
}

impl ErrorContext {
    /// The name of the ECS construct that failed.
    ///
    /// For systems, this is the system name.
    /// For commands, this is the call-site location string.
    pub fn name(&self) -> String {
        match self {
            Self::System { name, .. } => name.to_string(),
            Self::Condition { name, .. } => name.to_string(),
            Self::Observer { name, .. } => name.to_string(),
            Self::Command { name, .. } => name.to_string(),
        }
    }

    /// A string representation of the kind of ECS construct that failed.
    ///
    /// This is a simpler helper used for logging.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::System { .. } => "system",
            Self::Command { .. } => "command",
            Self::Condition { .. } => "condition",
            Self::Observer { .. } => "observer",
        }
    }
}
