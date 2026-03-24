//! Index-preserving set aliases and re-exports.
//!
//! [`IndexSet`] keeps insertion order while providing hash-based lookup,
//! based on `indexmap::set` with crate-aligned defaults.

// -----------------------------------------------------------------------------
// Modules

mod fixed;
// mod sparse;

// -----------------------------------------------------------------------------
// Re-Exports

pub use indexmap::set::{Difference, Drain, ExtractIf, Intersection};
pub use indexmap::set::{IntoIter, Iter, Splice, SymmetricDifference, Union};
pub use indexmap::set::{MutableValues, Slice};

// -----------------------------------------------------------------------------
// Exports

pub use fixed::IndexSet;
// pub use sparse::SparseIndexSet;
