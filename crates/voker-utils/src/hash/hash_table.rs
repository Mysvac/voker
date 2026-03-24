//! Re-exports `hashbrown::hash_table` APIs.
//!
//! This module exposes [`HashTable`] and related entry/iterator types so users
//! can access lower-level table operations directly when `HashMap` / `HashSet`
//! wrappers are not sufficient.

use hashbrown::hash_table as hb;

pub use hb::HashTable;

pub use hb::{AbsentEntry, Entry, OccupiedEntry, VacantEntry};
pub use hb::{Drain, ExtractIf, IntoIter};
pub use hb::{Iter, IterHash, IterHashMut, IterMut};
