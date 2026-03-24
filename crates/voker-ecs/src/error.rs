//! Error infrastructure used by ECS runtime execution.
//!
//! This module provides:
//! - [`EcsError`]: an owned, type-erased error container.
//! - [`ErrorContext`]: execution context captured for better diagnostics.
//! - [`ErrorHandler`]: pluggable handler signature used by schedules/systems.
//! - [`DefaultErrorHandler`]: resource wrapper storing the active global handler.
//!
//! The design keeps runtime APIs generic while preserving rich context for
//! logging and debugging.

use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::ToString;
use core::error::Error;
use core::fmt::{Debug, Display};
use core::ops::{Deref, DerefMut};
use core::panic::Location;

use crate::resource::Resource;
use crate::system::SystemName;
use crate::tick::Tick;
use crate::utils::Cloner;

// -----------------------------------------------------------------------------
// EcsError

/// Convenient result type for ECS operations.
pub type EcsResult<T> = Result<T, EcsError>;

// -----------------------------------------------------------------------------
// ECSResult

/// Type-erased ECS error container.
///
/// Stores any error that is `Send + Sync + 'static`, making it suitable for
/// passing across ECS boundaries and into configurable error handlers.
pub struct EcsError {
    error: Box<dyn Error + Send + Sync + 'static>,
}

// -----------------------------------------------------------------------------
// ErrorContext

/// Context for a [`EcsError`] to aid in debugging.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ErrorContext {
    System {
        name: SystemName,
        last_run: Tick,
    },
    Command {
        location: Location<'static>,
        this_run: Tick,
    },
}

// -----------------------------------------------------------------------------
// ErrorHandler

/// Function signature for ECS error handlers.
///
/// Receives the captured error and its execution context.
pub type ErrorHandler = fn(EcsError, ErrorContext);

// -----------------------------------------------------------------------------
// Implementation

// ----------------------------------------------------------
// EcsError

impl EcsError {
    /// Attempts to downcast this error to a concrete error type.
    ///
    /// Returns the concrete error on success, or the original [`EcsError`]
    /// unchanged on failure.
    pub fn downcast<E: Error + 'static>(self) -> Result<E, Self> {
        self.error
            .downcast::<E>()
            .map_err(|error| Self { error })
            .map(|error| *error)
    }

    /// Returns a shared reference to the concrete error type if it matches.
    pub fn downcast_ref<E: Error + 'static>(&self) -> Option<&E> {
        self.error.downcast_ref::<E>()
    }

    /// Returns a mutable reference to the concrete error type if it matches.
    pub fn downcast_mut<E: Error + 'static>(&mut self) -> Option<&mut E> {
        self.error.downcast_mut::<E>()
    }
}

impl<E: Error + Send + Sync + 'static> From<E> for EcsError {
    #[cold]
    fn from(error: E) -> Self {
        EcsError {
            error: Box::new(error),
        }
    }
}

impl Display for EcsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.error, f)
    }
}

impl Debug for EcsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.error, f)
    }
}

// ----------------------------------------------------------
// ErrorContext

impl ErrorContext {
    /// The name of the ECS construct that failed.
    ///
    /// For systems, this is the system name.
    /// For commands, this is the call-site location string.
    pub fn name(&self) -> Cow<'static, str> {
        match self {
            Self::System { name, .. } => Cow::Borrowed(name.as_str()),
            Self::Command { location, .. } => Cow::Owned(location.to_string()),
        }
    }

    /// A string representation of the kind of ECS construct that failed.
    ///
    /// This is a simpler helper used for logging.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::System { .. } => "system",
            Self::Command { .. } => "command",
        }
    }
}

// ----------------------------------------------------------
// Handler

/// Resource wrapper holding the active global ECS error handler.
///
/// The default handler is `panic`.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct DefaultErrorHandler(pub ErrorHandler);

impl Resource for DefaultErrorHandler {
    const MUTABLE: bool = true;
    const CLONER: Option<Cloner> = Some(Cloner::copyable::<Self>());
}

impl Default for DefaultErrorHandler {
    fn default() -> Self {
        Self(panic)
    }
}

impl Deref for DefaultErrorHandler {
    type Target = ErrorHandler;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DefaultErrorHandler {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

macro_rules! inner {
    ($call:path, $e:ident, $c:ident) => {
        $call!(
            "Encountered an error in {} `{}`: {}",
            $c.kind(),
            $c.name(),
            $e
        );
    };
}

/// Error handler that panics with the system error.
#[track_caller]
#[inline]
pub fn panic(error: EcsError, ctx: ErrorContext) {
    inner!(panic, error, ctx);
}

/// Error handler that logs the system error at the `error` level.
#[track_caller]
#[inline]
pub fn error(error: EcsError, ctx: ErrorContext) {
    inner!(log::error, error, ctx);
}

/// Error handler that logs the system error at the `warn` level.
#[track_caller]
#[inline]
pub fn warn(error: EcsError, ctx: ErrorContext) {
    inner!(log::warn, error, ctx);
}

/// Error handler that logs the system error at the `info` level.
#[track_caller]
#[inline]
pub fn info(error: EcsError, ctx: ErrorContext) {
    inner!(log::info, error, ctx);
}

/// Error handler that logs the system error at the `debug` level.
#[track_caller]
#[inline]
pub fn debug(error: EcsError, ctx: ErrorContext) {
    inner!(log::debug, error, ctx);
}

/// Error handler that logs the system error at the `trace` level.
#[track_caller]
#[inline]
pub fn trace(error: EcsError, ctx: ErrorContext) {
    inner!(log::trace, error, ctx);
}

/// Error handler that ignores the system error.
#[track_caller]
#[inline]
pub fn ignore(_: EcsError, _: ErrorContext) {}
