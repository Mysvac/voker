//! World runtime and entry-point APIs.
//!
//! The [`World`] is the root container of all ECS state: entities, components,
//! resources, schedules, and observers. It is the primary interface for both
//! game logic and engine internals.
//!
//! Key types:
//! - [`World`] — owns all ECS state; spawn, insert, remove, query
//! - [`DeferredWorld`] — a restricted view allowing structural mutations to be
//!   queued but not immediately applied (used inside hooks and observers)
//! - [`UnsafeWorld`] — raw pointer access used by the executor and low-level internals
//! - [`EntityRef`] / [`EntityMut`] / [`EntityOwned`] — scoped entity handles
//! - [`WorldId`] — a unique identifier for distinguishing worlds

// -----------------------------------------------------------------------------
// Modules

mod deferred;
mod entity;
mod from;
mod ident;
mod methods;
mod unsafe_world;
mod world;

// -----------------------------------------------------------------------------
// Exports

pub use deferred::DeferredWorld;
pub use entity::*;
pub use from::FromWorld;
pub use ident::WorldId;
pub use unsafe_world::UnsafeWorld;
pub use world::World;
