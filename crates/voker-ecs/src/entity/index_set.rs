//! Index set aliases specialized for [`Entity`] values.

use voker_utils::index::SparseIndexSet;

use super::Entity;

/// A sparse index set of [`Entity`] values.
pub type EntityIndexSet = SparseIndexSet<Entity>;

pub use voker_utils::index::set::{IntoIter, Iter};
