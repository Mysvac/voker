//! Hash-set aliases and re-exports.

// -----------------------------------------------------------------------------
// Modules

mod fixed;
mod noop;
mod sparse;

// -----------------------------------------------------------------------------
// Re-Exports

use hashbrown::hash_set as hb;

pub use hb::{Difference, Intersection, SymmetricDifference, Union};
pub use hb::{Drain, ExtractIf, IntoIter, Iter};
pub use hb::{Entry, OccupiedEntry, VacantEntry};

// -----------------------------------------------------------------------------
// Exports

pub use fixed::HashSet;
pub use noop::NoOpHashSet;
pub use sparse::SparseHashSet;
