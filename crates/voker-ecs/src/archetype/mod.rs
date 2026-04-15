//! Archetype topology and component-set grouping.
//!
//! Archetypes represent unique component-set layouts.
//!
//! Query prefiltering and entity storage routing rely on archetype
//! metadata, including archetype-to-table relationships.

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
