//! Deferred world-mutation command APIs.
//!
//! This module is split into three layers:
//! - [`Command`]: core command traits plus built-in free-function commands.
//! - [`Commands`]: high-level queued command writers used by systems.
//! - [`CommandQueue`]: the low-level type-erased byte queue used for deferred execution.
//!
//! In normal system execution, commands are enqueued first and applied at
//! deferred synchronization points.

mod command;
mod commands;
mod queue;

pub use command::*;
pub use commands::{Commands, EntityCommands};
pub use queue::CommandQueue;
