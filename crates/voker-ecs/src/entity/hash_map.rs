use voker_utils::hash::SparseHashMap;

use super::Entity;

pub type EntityHashMap<T> = SparseHashMap<Entity, T>;

pub use voker_utils::hash::map::{
    IntoIter, IntoKeys, IntoValues, Iter, IterMut, Keys, Values, ValuesMut,
};
