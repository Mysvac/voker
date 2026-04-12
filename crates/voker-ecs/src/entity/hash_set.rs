//! Hash set aliases specialized for [`Entity`] values.

use voker_utils::hash::SparseHashSet;

use super::Entity;

/// A sparse hash set of [`Entity`] values.
pub type EntityHashSet = SparseHashSet<Entity>;

pub use voker_utils::hash::set::{IntoIter, Iter};
