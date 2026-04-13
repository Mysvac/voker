//! World runtime and entry-point APIs.
//!
//! This module defines the central [`World`] type, world identifiers, low-level
//! access wrappers, and high-level mutation/query methods.
//!
//! Core submodules:
//! - [`world`]: storage owner and runtime counters.
//! - [`unsafe_world`]: raw access handle with explicit safety contracts.
//! - [`deferred`]: deferred command-oriented world facade.
//! - [`entity`]: entity fetch/view helpers used by world APIs.
//! - [`methods`]: high-level convenience methods implemented on [`World`].
//!
//! The low-level [`UnsafeWorld`] wrapper distinguishes between data-only mutable
//! access and full structural mutation. Internal systems use this split to keep
//! unsafe aliasing boundaries explicit.

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
