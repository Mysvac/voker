// -----------------------------------------------------------------------------
// Modules

mod fixed;
mod sparse;

// -----------------------------------------------------------------------------
// Re-Exports

pub use indexmap::map::{Drain, ExtractIf, IntoIter, IntoKeys, IntoValues};
pub use indexmap::map::{Entry, IndexedEntry, OccupiedEntry, VacantEntry};
pub use indexmap::map::{Iter, IterMut, IterMut2, Keys, Splice, Values, ValuesMut};
pub use indexmap::map::{MutableEntryKey, MutableKeys, Slice};

// -----------------------------------------------------------------------------
// Exports

pub use fixed::IndexMap;
pub use sparse::SparseIndexMap;
