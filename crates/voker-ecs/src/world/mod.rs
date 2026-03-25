//! World runtime and entry-point APIs.
//!
//! This module defines the central [`World`] type, world identifiers, low-level
//! access wrappers, and high-level mutation/query methods.
//!
//! The low-level [`UnsafeWorld`] wrapper distinguishes between data-only mutable
//! access and full structural mutation. Internal systems use this split to keep
//! unsafe aliasing boundaries explicit.

// -----------------------------------------------------------------------------
// Modules

mod entity;
mod from;
mod ident;
mod methods;
mod unsafe_world;
mod world;

// -----------------------------------------------------------------------------
// Exports

pub use entity::*;
pub use from::FromWorld;
pub use ident::WorldId;
pub use unsafe_world::UnsafeWorld;
pub use world::World;
