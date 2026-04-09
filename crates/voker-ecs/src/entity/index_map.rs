use voker_utils::index::SparseIndexMap;

use super::Entity;

pub type EntityIndexMap<T> = SparseIndexMap<Entity, T>;

pub use voker_utils::index::map::{
    IntoIter, IntoKeys, IntoValues, Iter, IterMut, Keys, Values, ValuesMut,
};
