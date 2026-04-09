//! Hash primitives and container aliases.
//!
//! This module re-exports `hashbrown` / `foldhash` and provides crate-level
//! hash builders plus map/set aliases for common usage patterns.

// -----------------------------------------------------------------------------
// Modules

mod hasher;

pub mod map;
pub mod set;
pub mod table;

// -----------------------------------------------------------------------------
// Exports

pub use hasher::{FixedHashState, FixedHasher};
pub use hasher::{NoOpHashState, NoOpHasher};
pub use hasher::{SparseHashState, SparseHasher};

pub use map::{HashMap, NoOpHashMap, SparseHashMap};
pub use set::{HashSet, NoOpHashSet, SparseHashSet};
pub use table::HashTable;

pub use hashbrown::Equivalent;

// -----------------------------------------------------------------------------
// Re-export crates

pub use foldhash;
pub use hashbrown;
