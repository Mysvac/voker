//! High-level [`World`] operations.
//!
//! This module is split by domain:
//! - archetype inspection,
//! - entity spawn/despawn,
//! - query creation,
//! - registration helpers,
//! - resource insertion/removal/access.

mod arche;
mod despawn;
mod query;
mod register;
mod resource;
mod spawn;
