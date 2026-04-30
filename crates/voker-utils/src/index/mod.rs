//! Index-preserving containers based on `indexmap`.
//!
//! This module exposes map/set aliases that keep insertion order while
//! following crate-wide hash-state defaults.

pub use indexmap::{Equivalent, GetDisjointMutError, TryReserveError};

pub use indexmap;

pub mod map;
pub mod set;

pub use map::IndexMap;
pub use set::IndexSet;

pub use map::SparseIndexMap;
pub use set::SparseIndexSet;
