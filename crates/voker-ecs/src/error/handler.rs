use core::ops::{Deref, DerefMut};
use voker_ecs_derive::Resource;

use super::{EcsError, ErrorContext, Severity};

/// Function signature for ECS error handlers.
///
/// Receives the captured error and its execution context.
pub type ErrorHandler = fn(EcsError, ErrorContext);

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
#[track_caller]
pub fn match_severity(err: EcsError, ctx: ErrorContext) {
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

/// Error handler that panics with the system error.
#[inline]
#[track_caller]
pub fn panic(error: EcsError, ctx: ErrorContext) {
    inner!(panic, error, ctx);
}

/// Error handler that logs the system error at the `error` level.
#[inline]
#[track_caller]
pub fn error(error: EcsError, ctx: ErrorContext) {
    inner!(log::error, error, ctx);
}

/// Error handler that logs the system error at the `warn` level.
#[inline]
#[track_caller]
pub fn warn(error: EcsError, ctx: ErrorContext) {
    inner!(log::warn, error, ctx);
}

/// Error handler that logs the system error at the `info` level.
#[inline]
#[track_caller]
pub fn info(error: EcsError, ctx: ErrorContext) {
    inner!(log::info, error, ctx);
}

/// Error handler that logs the system error at the `debug` level.
#[inline]
#[track_caller]
pub fn debug(error: EcsError, ctx: ErrorContext) {
    inner!(log::debug, error, ctx);
}

/// Error handler that logs the system error at the `trace` level.
#[inline]
#[track_caller]
pub fn trace(error: EcsError, ctx: ErrorContext) {
    inner!(log::trace, error, ctx);
}

/// Error handler that ignores the system error.
#[inline]
#[track_caller]
pub fn ignore(_: EcsError, _: ErrorContext) {}
