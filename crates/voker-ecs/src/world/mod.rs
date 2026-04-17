//! World runtime and entry-point APIs.
//!
//! This module defines the central [`World`] type, world identifiers, low-level
//! access wrappers, and high-level mutation/query methods.

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
