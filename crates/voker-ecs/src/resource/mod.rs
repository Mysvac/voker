//! Global resources — singleton data attached to the world.
//!
//! This module covers the **registration** layer only:
//! - [`Resource`] — the marker trait types must implement.
//! - [`ResourceId`] / [`ResourceInfo`] — stable identity and type metadata.
//! - [`Resources`] — the per-world registry that maps `TypeId → ResourceId`.
//!
//! The actual **storage** (raw bytes and change-detection ticks) lives in
//! [`crate::storage::ResourceStorage`], which is indexed by `ResourceId`.
//! World methods (`insert_resource`, `get_resource`, etc.) coordinate between
//! both layers.

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
