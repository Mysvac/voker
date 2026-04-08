//! Global resources unrelated to entities.
//!
//! Resources are singleton-like values stored per world.
//! This module defines resource identity/metadata and resource containers,
//! plus derive support for user-defined resource types.

// -----------------------------------------------------------------------------
// Modules

mod ident;
mod info;
mod resource;
mod resources;

// -----------------------------------------------------------------------------
// Exports

pub use voker_ecs_derive::Resource;

pub use ident::ResourceId;
pub use info::{ResourceDescriptor, ResourceInfo};
pub use resource::Resource;
pub use resources::Resources;
