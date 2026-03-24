//! World runtime and entry-point APIs.
//!
//! This module defines the central [`World`] type, world identifiers, low-level
//! access wrappers, and high-level mutation/query methods.

// -----------------------------------------------------------------------------
// Modules

mod access;
mod ident;
mod methods;
mod unsafe_world;
mod world;

// -----------------------------------------------------------------------------
// Exports

pub use access::*;
pub use ident::{WorldId, WorldIdAllocator};
pub use unsafe_world::UnsafeWorld;
pub use world::World;
