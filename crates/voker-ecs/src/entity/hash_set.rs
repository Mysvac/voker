use voker_utils::hash::SparseHashSet;

use super::Entity;

pub type EntityHashSet = SparseHashSet<Entity>;

pub use voker_utils::hash::set::{IntoIter, Iter};
