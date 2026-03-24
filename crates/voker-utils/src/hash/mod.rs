//! Hash primitives and container aliases.
//!
//! This module re-exports `hashbrown` / `foldhash` and provides crate-level
//! hash builders plus map/set aliases for common usage patterns.

// -----------------------------------------------------------------------------
// Modules

mod hasher;

pub mod hash_map;
pub mod hash_set;
pub mod hash_table;

// -----------------------------------------------------------------------------
// Exports

pub use hasher::{FixedHashState, FixedHasher};
pub use hasher::{NoOpHashState, NoOpHasher};
pub use hasher::{SparseHashState, SparseHasher};

pub use hash_map::{HashMap, NoOpHashMap, SparseHashMap};
pub use hash_set::{HashSet, NoOpHashSet, SparseHashSet};
pub use hash_table::HashTable;

pub use hashbrown::Equivalent;

// -----------------------------------------------------------------------------
// Re-export crates

pub use foldhash;
pub use hashbrown;
