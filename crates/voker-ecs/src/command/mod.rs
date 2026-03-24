//! Deferred world-mutation command APIs.
//!
//! This module provides the command queue model used by systems that cannot or
//! should not mutate [`World`](crate::world::World) directly.
//!
//! - [`Commands`] is the main enqueue interface used inside systems.
//! - [`EntityCommands`] scopes commands to a specific entity.
//! - [`CommandQueue`] stores command objects until flush/apply.
//! - [`CommandObject`] is the type-erased command payload.

mod commands;
mod entity;
mod object;
mod queue;

pub use commands::Commands;
pub use entity::EntityCommands;
pub use object::CommandObject;
pub use queue::CommandQueue;
