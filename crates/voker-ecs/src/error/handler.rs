use core::ops::{Deref, DerefMut};
use voker_ecs_derive::Resource;

use super::{ErrorContext, GameError, Severity};

/// Function signature for ECS error handlers.
///
/// Receives the captured error and its execution context.
///
/// This is used by schedule executors and command application paths when
/// fallible work returns a [`GameError`].
pub type ErrorHandler = fn(GameError, ErrorContext);

/// Resource wrapper for the fallback ECS error handler.
///
/// When no per-call handler is provided, runtime errors are routed through
/// this handler.
#[derive(Resource, Debug, Clone, Copy)]
pub struct FallbackErrorHandler(pub ErrorHandler);

impl Default for FallbackErrorHandler {
    fn default() -> Self {
        Self(match_severity)
    }
}

impl Deref for FallbackErrorHandler {
    type Target = ErrorHandler;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FallbackErrorHandler {
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

/// Error handler that defers to an error's [`Severity`].
///
/// Dispatch table:
/// - [`Severity::Ignore`] => [`ignore`]
/// - [`Severity::Trace`] => [`trace`]
/// - [`Severity::Debug`] => [`debug`]
/// - [`Severity::Info`] => [`info`]
/// - [`Severity::Warning`] => [`warn`]
/// - [`Severity::Error`] => [`error`]
/// - [`Severity::Panic`] => [`panic()`]
#[track_caller]
pub fn match_severity(err: GameError, ctx: ErrorContext) {
    match err.severity() {
        Severity::Ignore => ignore(err, ctx),
        Severity::Trace => trace(err, ctx),
        Severity::Debug => debug(err, ctx),
        Severity::Info => info(err, ctx),
        Severity::Warning => warn(err, ctx),
        Severity::Error => error(err, ctx),
        Severity::Panic => panic(err, ctx),
    }
}

/// Error handler that panics with the formatted error message.
#[inline]
#[track_caller]
pub fn panic(error: GameError, ctx: ErrorContext) {
    inner!(panic, error, ctx);
}

/// Error handler that logs the error at the `error` level.
#[inline]
#[track_caller]
pub fn error(error: GameError, ctx: ErrorContext) {
    inner!(log::error, error, ctx);
}

/// Error handler that logs the error at the `warn` level.
#[inline]
#[track_caller]
pub fn warn(error: GameError, ctx: ErrorContext) {
    inner!(log::warn, error, ctx);
}

/// Error handler that logs the error at the `info` level.
#[inline]
#[track_caller]
pub fn info(error: GameError, ctx: ErrorContext) {
    inner!(log::info, error, ctx);
}

/// Error handler that logs the error at the `debug` level.
#[inline]
#[track_caller]
pub fn debug(error: GameError, ctx: ErrorContext) {
    inner!(log::debug, error, ctx);
}

/// Error handler that logs the error at the `trace` level.
#[inline]
#[track_caller]
pub fn trace(error: GameError, ctx: ErrorContext) {
    inner!(log::trace, error, ctx);
}

/// Error handler that ignores the error.
#[inline]
#[track_caller]
pub fn ignore(_: GameError, _: ErrorContext) {}
