//! Archetype topology and component-set grouping.
//!
//! An [`Archetype`] represents a unique combination of component types.
//! Every entity belongs to exactly one archetype at any time. When components
//! are added or removed, the entity migrates to a new archetype.
//!
//! Key responsibilities:
//! - routing entities to the correct storage table or sparse maps,
//! - caching component lifecycle hooks (add, insert, remove, discard, despawn),
//! - recording observer presence flags so dispatchers can skip empty sets,
//! - caching archetype-transition results (`BundleId → ArcheId`) to avoid
//!   repeated lookups on common insert/remove patterns.

// -----------------------------------------------------------------------------
// Modules

mod arches;
mod ident;
mod info;

// -----------------------------------------------------------------------------
// Exports

pub use arches::Archetypes;
pub use ident::{ArcheId, ArcheRow};
pub use info::Archetype;

pub(crate) use info::ObserverFlags;
