use voker_utils::index::SparseIndexSet;

use super::Entity;

pub type EntityIndexSet = SparseIndexSet<Entity>;

pub use voker_utils::index::set::{IntoIter, Iter};
