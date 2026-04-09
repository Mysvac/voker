//! Entity identifiers, allocation, and location tracking.
//!
//! Entities are stable handles composed from:
//! - an index-like identifier,
//! - a generation counter for stale-handle protection.
//!
//! This module also provides allocator and storage-location helpers used by the
//! world/archetype systems.

// -----------------------------------------------------------------------------
// Modules

mod allocator;
mod error;
mod ident;
mod info;
mod mapper;
mod storage;

pub mod hash_map;
pub mod hash_set;
pub mod index_map;
pub mod index_set;

// -----------------------------------------------------------------------------
// Exports

pub use allocator::{AllocEntitiesIter, EntityAllocator, RemoteAllocator};
pub use error::*;
pub use hash_map::EntityHashMap;
pub use hash_set::EntityHashSet;
pub use ident::{Entity, EntityId, EntityTag};
pub use index_map::EntityIndexMap;
pub use index_set::EntityIndexSet;
pub use info::{Entities, EntityLocation, MovedEntityRow};
pub use mapper::{EntityMapper, MapEntities, SceneEntityMapper};
pub use storage::StorageId;
