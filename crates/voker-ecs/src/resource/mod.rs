//! Global resources unrelated to entities.
//!
//! Resources are singleton-like values stored per world.
//! This module defines resource identity/metadata and resource containers,
//! plus derive support for user-defined resource types.

// -----------------------------------------------------------------------------
// Modules

mod ident;
mod impls;
mod info;
mod resources;

// -----------------------------------------------------------------------------
// Exports

pub use voker_ecs_derive::Resource;

pub use ident::ResourceId;
pub use impls::Resource;
pub use info::{ResourceDescriptor, ResourceInfo};
pub use resources::Resources;
