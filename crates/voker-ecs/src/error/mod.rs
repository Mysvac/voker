//! Error types and routing policies used by ECS execution.
//!
//! Runtime failures in systems, conditions, observers, and commands are modeled
//! as [`GameError`] values. Each error carries a [`Severity`] used by handlers to
//! decide whether to ignore, log, or panic.
//!
//! [`ErrorContext`] adds call-site metadata (for example, which system failed)
//! and is passed together with the error to an [`ErrorHandler`].
//!
//! By default, [`FallbackErrorHandler`] dispatches through [`match_severity`].
//!
//! # Main Types
//! - [`GameError`]: type-erased error + severity metadata.
//! - [`Severity`]: advisory handling level.
//! - [`ErrorContext`]: where the error happened.
//! - [`ErrorHandler`]: function pointer used to process errors.
//!
//! # Result Alias
//! [`GameResult`] is the crate-local alias for `Result<T, GameError>`.
//!
//! [`GameError`]: struct@GameError`

mod context;
mod game_error;
mod handler;
mod impls;

pub use context::*;
pub use game_error::*;
pub use handler::*;
pub use voker_ecs_derive::GameError;

/// Convenience alias for fallible ECS APIs returning `GameError`.
pub type GameResult<T> = Result<T, GameError>;
