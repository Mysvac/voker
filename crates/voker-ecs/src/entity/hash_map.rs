//! Hash map aliases specialized for [`Entity`] keys.

use voker_utils::hash::SparseHashMap;

use super::Entity;

/// A sparse hash map keyed by [`Entity`].
pub type EntityHashMap<T> = SparseHashMap<Entity, T>;

pub use voker_utils::hash::map::{
    IntoIter, IntoKeys, IntoValues, Iter, IterMut, Keys, Values, ValuesMut,
};
