use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
use core::fmt;

use voker_utils::hash::{HashTable, hash_table};

use super::{impl_dynamic_type_info, impl_dynamic_type_path};
use crate::Reflect;
use crate::info::TypeInfo;
use crate::ops::{ApplyError, ReflectCloneError};

// -----------------------------------------------------------------------------
// Dynamic Map

/// A dynamic container representing a map-like collection.
///
/// `DynamicMap` is a type-erased dynamic map that can store key-value pairs where
/// both keys and values implement [`Reflect`]. It represents associative collections
/// like `HashMap` or `BTreeMap` in Rust.
///
/// # Type Information
///
/// Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`](crate::info::OpaqueInfo),
/// but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
///
/// A `DynamicMap` can optionally represent a specific map type through its
/// [`represented_type_info`]. When set, this allows the dynamic map to be treated
/// as if it were a specific static map type for reflection purposes.
///
/// # Key Requirements
///
/// Keys in a `DynamicMap` must support:
/// - Hashing via [`Reflect::reflect_hash`]
/// - Equality comparison via [`Reflect::reflect_eq`]
/// - Self-equality (a key must be equal to itself)
///
/// # Examples
///
/// ## Creating and populating a dynamic map
///
/// ```
/// use voker_reflect::ops::{Map, DynamicMap};
///
/// let mut map = DynamicMap::new();
/// map.extend("key1", "value1");
/// map.extend("key2", "value2");
/// map.extend("key3", 42);
///
/// assert_eq!(map.len(), 3);
/// ```
///
/// ## Looking up values
///
/// ```
/// use voker_reflect::ops::{Map, DynamicMap};
///
/// let mut map = DynamicMap::new();
/// map.extend("counter", 100_i32);
///
/// // Get as dynamic reference
/// if let Some(value) = map.get(&"counter") {
///     println!("Found: {:?}", value);
/// }
/// ```
///
/// [`reflect_kind`]: Reflect::reflect_kind
/// [`reflect_ref`]: Reflect::reflect_ref
/// [`represented_type_info`]: Reflect::represented_type_info
#[derive(Default)]
pub struct DynamicMap {
    info: Option<&'static TypeInfo>,
    hash_table: HashTable<(Box<dyn Reflect>, Box<dyn Reflect>)>,
}

impl_dynamic_type_path!(DynamicMap);
impl_dynamic_type_info!(DynamicMap);

impl DynamicMap {
    /// Creates an empty `DynamicMap`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::DynamicMap;
    /// let map = DynamicMap::new();
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            info: None,
            hash_table: HashTable::new(),
        }
    }

    /// Creates a new empty `DynamicMap` with at least the specified capacity.
    ///
    /// This can be used to avoid reallocations when you know approximately
    /// how many key-value pairs will be added to the map.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            info: None,
            hash_table: HashTable::with_capacity(capacity),
        }
    }

    /// Sets the [`TypeInfo`] that this dynamic map represents.
    ///
    /// When set, [`Reflect::represented_type_info`] will return this information,
    /// allowing the dynamic map to be treated as if it were a specific static map type.
    ///
    /// # Panics
    ///
    /// Panics if `info` is `Some` but does not contain map type information.
    pub const fn set_type_info(&mut self, info: Option<&'static TypeInfo>) {
        match info {
            Some(info) => {
                assert!(info.kind().is_map(), "`TypeInfo` mismatched.");
                self.info = Some(info);
            }
            None => {
                self.info = None;
            }
        }
    }

    /// Inserts a boxed key-value pair into the map.
    ///
    /// This is the low-level version of [`extend`] that accepts already-boxed values.
    ///
    /// # Returns
    ///
    /// - `Some(old_value)` if the key already existed in the map (the old value is replaced)
    /// - `None` if the key did not exist in the map
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The key does not support [`Reflect::reflect_hash`]
    /// - The key does not support [`Reflect::reflect_eq`]
    /// - The key is not equal to itself (violates reflexivity)
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::DynamicMap;
    /// let mut map = DynamicMap::new();
    /// let old_value = map.extend_boxed(Box::new("key1"), Box::new("value1"));
    /// assert!(old_value.is_none());
    ///
    /// let old_value = map.extend_boxed(Box::new("key1"), Box::new("value2"));
    /// assert!(old_value.is_some());
    /// ```
    ///
    /// [`extend`]: DynamicMap::extend
    pub fn extend_boxed(
        &mut self,
        key: Box<dyn Reflect>,
        value: Box<dyn Reflect>,
    ) -> Option<Box<dyn Reflect>> {
        debug_assert_eq!(
            key.reflect_eq(&*key),
            Some(true),
            "The key is not `reflect_eq` to itself: `{}`.",
            key.reflect_type_path(),
        );

        let hash = Self::internal_hash(&*key);
        let eq = Self::internal_eq(&*key);
        match self.hash_table.find_mut(hash, eq) {
            Some((_, old)) => Some(core::mem::replace(old, value)),
            None => {
                self.hash_table.insert_unique(
                    Self::internal_hash(key.as_ref()),
                    (key, value),
                    |(key, _)| Self::internal_hash(&**key),
                );
                None
            }
        }
    }

    /// Inserts a key-value pair into the map.
    ///
    /// This is a convenience method that boxes the key and value and calls
    /// [`extend_boxed`].
    ///
    /// # Returns
    ///
    /// - `Some(old_value)` if the key already existed in the map (the old value is replaced)
    /// - `None` if the key did not exist in the map
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - The key does not support [`Reflect::reflect_hash`]
    /// - The key does not support [`Reflect::reflect_eq`]
    /// - The key is not equal to itself (violates reflexivity)
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::ops::{Map, DynamicMap};
    /// let mut map = DynamicMap::new();
    /// map.extend("name", "Alice");
    /// map.extend("age", 30_i32);
    /// map.extend("active", true);
    ///
    /// assert_eq!(map.len(), 3);
    /// ```
    ///
    /// [`extend_boxed`]: DynamicMap::extend_boxed
    #[inline]
    pub fn extend<K: Reflect, V: Reflect>(&mut self, key: K, value: V) -> Option<Box<dyn Reflect>> {
        self.extend_boxed(Box::new(key), Box::new(value))
    }

    /// Computes the hash of a value for internal use.
    ///
    /// # Panics
    ///
    /// Panics if the value does not support `reflect_hash`.
    fn internal_hash(value: &dyn Reflect) -> u64 {
        value.reflect_hash().unwrap_or_else(|| {
            panic!(
                "the given value of type `{}` does not support reflect hashing",
                value.reflect_type_path(),
            );
        })
    }

    /// Creates an equality comparison function for a key.
    fn internal_eq(
        key: &dyn Reflect,
    ) -> impl FnMut(&(Box<dyn Reflect>, Box<dyn Reflect>)) -> bool + '_ {
        |(other, _)| {
            key.reflect_eq(&**other).unwrap_or_else(|| {
                panic!(
                    "the given value of type `{}` does not support reflect hashing",
                    other.reflect_type_path(),
                )
            })
        }
    }
}

impl Reflect for DynamicMap {
    crate::reflection::impl_reflect_cast_fn!(Map);

    #[inline]
    fn is_dynamic(&self) -> bool {
        true
    }

    #[inline]
    fn represented_type_info(&self) -> Option<&'static TypeInfo> {
        self.info
    }

    #[inline]
    fn to_dynamic(&self) -> Box<dyn Reflect> {
        Box::new(<Self as Map>::to_dynamic_map(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        Ok(Box::new(<Self as Map>::to_dynamic_map(self)))
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::map_apply(self, value)
    }

    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        crate::impls::map_eq(self, other)
    }

    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        crate::impls::map_cmp(self, other)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::map_hash(self)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynamicMap(")?;
        crate::impls::map_debug(self, f)?;
        write!(f, ")")
    }
}

impl fmt::Debug for DynamicMap {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reflect_debug(f)
    }
}

impl FromIterator<(Box<dyn Reflect>, Box<dyn Reflect>)> for DynamicMap {
    #[inline]
    fn from_iter<I: IntoIterator<Item = (Box<dyn Reflect>, Box<dyn Reflect>)>>(items: I) -> Self {
        let mut this = DynamicMap::new();
        for (key, value) in items.into_iter() {
            this.extend_boxed(key, value);
        }
        this
    }
}

impl<K: Reflect, V: Reflect> FromIterator<(K, V)> for DynamicMap {
    #[inline]
    fn from_iter<I: IntoIterator<Item = (K, V)>>(items: I) -> Self {
        let mut this = DynamicMap::new();
        for (key, value) in items.into_iter() {
            this.extend_boxed(Box::new(key), Box::new(value));
        }
        this
    }
}

impl IntoIterator for DynamicMap {
    type Item = (Box<dyn Reflect>, Box<dyn Reflect>);
    type IntoIter = hash_table::IntoIter<Self::Item>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.hash_table.into_iter()
    }
}

impl<'a> IntoIterator for &'a DynamicMap {
    type Item = (&'a dyn Reflect, &'a dyn Reflect);
    type IntoIter = core::iter::Map<
        hash_table::Iter<'a, (Box<dyn Reflect>, Box<dyn Reflect>)>,
        fn(&'a (Box<dyn Reflect>, Box<dyn Reflect>)) -> Self::Item,
    >;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.hash_table.iter().map(|(k, v)| (&**k, &**v))
    }
}

// -----------------------------------------------------------------------------
// Map trait

/// A trait for type-erased map-like operations via reflection.
///
/// This trait represents any associative collection that maps keys to values, including:
/// - Hash maps (`HashMap<K, V>`)
/// - B-tree maps (`BTreeMap<K, V>`)
/// - Other key-value collections
///
/// # Key Requirements
///
/// Implementors must ensure that keys:
/// 1. Support hashing via [`Reflect::reflect_hash`]
/// 2. Support equality comparison via [`Reflect::reflect_eq`]
/// 3. Are equal to themselves (reflexivity)
/// 4. Have consistent hashing (equal keys have equal hashes)
///
/// The ordering of entries is not guaranteed by this trait and may vary by implementation.
///
/// # Examples
///
/// ```
/// use voker_reflect::{Reflect, ops::Map};
/// use std::collections::BTreeMap;
///
/// let mut map = BTreeMap::new();
/// map.insert("a", 1);
/// map.insert("b", 2);
///
/// let map_ref: &mut dyn Map = &mut map;
/// assert_eq!(map_ref.len(), 2);
///
/// if let Some(value) = map_ref.get_as::<i32>(&"b") {
///     assert_eq!(value, &2);
/// }
/// ```
pub trait Map: Reflect {
    /// Returns a reference to the value associated with the given key.
    ///
    /// Returns `None` if the key is not present in the map or incompitable.
    ///
    /// For type-safe access when the k-v type is known,
    /// use `<dyn Map>::get_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Map};
    /// # use std::collections::BTreeMap;
    /// let mut map = BTreeMap::new();
    /// map.insert("key", 42);
    /// let map_ref: &dyn Map = &map;
    ///
    /// assert!(map_ref.get(&"key").is_some());
    /// assert!(map_ref.get(&"missing").is_none());
    /// ```
    fn get(&self, key: &dyn Reflect) -> Option<&dyn Reflect>;

    /// Returns a mutable reference to the value associated with the given key.
    ///
    /// Returns `None` if the key is not present in the map or incompitable.
    ///
    /// For type-safe access when the k-v type is known,
    /// use `<dyn Map>::get_as` instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Map};
    /// # use std::collections::BTreeMap;
    /// let mut map = BTreeMap::new();
    /// map.insert("counter", 0_i32);
    /// let map_ref: &mut dyn Map = &mut map;
    ///
    /// if let Some(value) = map_ref.get_mut(&"counter") {
    ///     *value.downcast_mut::<i32>().unwrap() += 1;
    /// }
    ///
    /// assert_eq!(map.get(&"counter"), Some(&1));
    /// ```
    fn get_mut(&mut self, key: &dyn Reflect) -> Option<&mut dyn Reflect>;

    /// Returns the number of key-value pairs in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Map};
    /// # use std::collections::BTreeMap;
    /// let map: BTreeMap<&str, i32> = [("a", 1), ("b", 2), ("c", 3)]
    ///     .into_iter()
    ///     .collect();
    /// let map_ref: &dyn Map = &map;
    ///
    /// assert_eq!(map_ref.len(), 3);
    /// ```
    fn len(&self) -> usize;

    /// Returns `true` if the list contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Map};
    /// # use std::collections::BTreeMap;
    /// let empty: BTreeMap<&str, i32> = BTreeMap::new();
    /// let empty_ref: &dyn Map = &empty;
    /// assert!(empty_ref.is_empty());
    /// ```
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the key-value pairs of the map.
    ///
    /// The iteration order is not specified and may vary between implementations.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Map};
    /// # use std::collections::BTreeMap;
    ///
    /// let map: BTreeMap<&str, i32> = [("a", 1), ("b", 2), ("c", 3)]
    ///     .into_iter()
    ///     .collect();
    /// let map_ref: &dyn Map = &map;
    ///
    /// let sum: i32 = map_ref.iter()
    ///     .filter_map(|(_, v)| v.downcast_ref::<i32>())
    ///     .sum();
    ///
    /// assert_eq!(sum, 6);
    /// ```
    fn iter(&self) -> Box<dyn Iterator<Item = (&dyn Reflect, &dyn Reflect)> + '_>;

    /// Removes all key-value pairs from the map and returns them as a vector.
    ///
    /// After calling this method, the map will be empty. The order of the
    /// returned pairs is not specified.
    fn drain(&mut self) -> Vec<(Box<dyn Reflect>, Box<dyn Reflect>)>;

    /// Retains only the key-value pairs specified by the predicate.
    ///
    /// Remove all pairs `(k, v)` for which `f(&k, &mut v)` returns `false`.
    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect, &mut dyn Reflect) -> bool);

    /// Creates a new [`DynamicMap`] from this map.
    ///
    /// This method converts the map to a dynamic representation.
    ///
    /// For keys, it attempts to clone them using [`Reflect::reflect_clone`] if possible,
    /// otherwise falls back to converting them to dynamic types.
    fn to_dynamic_map(&self) -> DynamicMap {
        let mut map = DynamicMap::with_capacity(self.len());
        map.set_type_info(self.represented_type_info());
        for (key, value) in self.iter() {
            if let Ok(k) = key.reflect_clone() {
                debug_assert_eq!(
                    (*k).type_id(),
                    key.type_id(),
                    "`Reflect::reflect_clone` should return the same type: {}",
                    value.reflect_type_path(),
                );
                map.insert(k, value.to_dynamic());
            } else {
                map.insert(key.to_dynamic(), value.to_dynamic());
            }
        }
        map
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map already contained a value for the key, the old value is returned.
    ///
    /// In standard implementation (e.g. `BTreeMap<K, V>`), this function will use
    /// [`FromReflect::take_from_reflect`] to convert key and value.
    ///
    /// # Panics
    ///
    /// May panic if:
    /// - The key type is incompatible with the map
    /// - The key does not support required operations (hashing, equality)
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Map};
    /// # use std::collections::BTreeMap;
    /// let mut map = <BTreeMap<&str, i32>>::new();
    /// let map_ref: &mut dyn Map = &mut map;
    ///
    /// // Insert new key
    /// let result = map_ref.insert(Box::new("key"), Box::new(1_i32));
    /// assert!(result.is_none());
    ///
    /// // Replace existing key
    /// let result = map_ref.insert(Box::new("key"), Box::new(2_i32));
    /// assert_eq!(result.unwrap().downcast_ref::<i32>(), Some(&1));
    /// ```
    ///
    /// [`FromReflect::take_from_reflect`]: crate::FromReflect::take_from_reflect
    fn insert(
        &mut self,
        key: Box<dyn Reflect>,
        value: Box<dyn Reflect>,
    ) -> Option<Box<dyn Reflect>>;

    /// Attempts to insert a key-value pair into the map.
    ///
    /// This only requires checking whether the types match.
    ///
    /// In standard implementation (e.g. `BTreeMap<K, V>`), this function will use
    /// [`FromReflect::take_from_reflect`] to convert key and value.
    ///
    /// # Panics
    /// May panic if the key does not support required operations (hashing, equality).
    ///
    /// # Returns
    ///
    /// - `Ok(Some(old_value))` if the key already existed (old value replaced)
    /// - `Ok(None)` if the key did not exist (new entry inserted)
    /// - `Err((key, value))` if the key type is incompatible
    ///
    /// [`FromReflect::take_from_reflect`]: crate::FromReflect::take_from_reflect
    fn try_insert(
        &mut self,
        key: Box<dyn Reflect>,
        value: Box<dyn Reflect>,
    ) -> Result<Option<Box<dyn Reflect>>, (Box<dyn Reflect>, Box<dyn Reflect>)>;

    /// Removes a key from the map, returning the value if the key was previously in the map.
    ///
    /// # Returns
    ///
    /// - `None` if the key type is incompitable.
    /// - `Some(value)` if the key was present in the map
    /// - `None` if the key was not present in the map
    ///
    /// # Panics
    ///
    /// May panic if the key does not support required operations (hashing, equality)
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_reflect::{Reflect, ops::Map};
    /// # use std::collections::BTreeMap;
    /// let mut map = BTreeMap::new();
    /// map.insert("key", "value");
    /// let map_ref: &mut dyn Map = &mut map;
    ///
    /// let removed = map_ref.remove(&"key");
    /// assert!(removed.is_some());
    /// assert!(map_ref.is_empty());
    /// ```
    fn remove(&mut self, key: &dyn Reflect) -> Option<Box<dyn Reflect>>;
}

impl Map for DynamicMap {
    #[inline]
    fn get(&self, key: &dyn Reflect) -> Option<&dyn Reflect> {
        self.hash_table
            .find(Self::internal_hash(key), Self::internal_eq(key))
            .map(|(_, value)| &**value)
    }

    #[inline]
    fn get_mut(&mut self, key: &dyn Reflect) -> Option<&mut dyn Reflect> {
        self.hash_table
            .find_mut(Self::internal_hash(key), Self::internal_eq(key))
            .map(|(_, value)| &mut **value)
    }

    #[inline]
    fn len(&self) -> usize {
        self.hash_table.len()
    }

    #[inline]
    fn iter(&self) -> Box<dyn Iterator<Item = (&dyn Reflect, &dyn Reflect)> + '_> {
        let iter = self.hash_table.iter().map(|(k, v)| (&**k, &**v));
        Box::new(iter)
    }

    #[inline]
    fn drain(&mut self) -> Vec<(Box<dyn Reflect>, Box<dyn Reflect>)> {
        self.hash_table.drain().collect()
    }

    #[inline]
    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect, &mut dyn Reflect) -> bool) {
        self.hash_table.retain(move |(key, value)| f(&**key, &mut **value));
    }

    #[inline]
    fn insert(
        &mut self,
        key: Box<dyn Reflect>,
        value: Box<dyn Reflect>,
    ) -> Option<Box<dyn Reflect>> {
        self.extend_boxed(key, value)
    }

    #[inline]
    fn try_insert(
        &mut self,
        key: Box<dyn Reflect>,
        value: Box<dyn Reflect>,
    ) -> Result<Option<Box<dyn Reflect>>, (Box<dyn Reflect>, Box<dyn Reflect>)> {
        Ok(self.extend_boxed(key, value))
    }

    fn remove(&mut self, key: &dyn Reflect) -> Option<Box<dyn Reflect>> {
        let hash = Self::internal_hash(key);
        let eq = Self::internal_eq(key);
        match self.hash_table.find_entry(hash, eq) {
            Ok(entry) => {
                let ((_, old_value), _) = entry.remove();
                Some(old_value)
            }
            Err(_) => None,
        }
    }
}

impl dyn Map {
    /// Returns a typed reference to the value associated with the given key.
    ///
    /// Returns `None` if:
    /// - The key is not present in the map
    /// - The key type is incompatible with the map
    /// - The value cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::{Reflect, ops::Map};
    /// use std::collections::BTreeMap;
    ///
    /// let map: BTreeMap<&str, i32> = [("count", 42)].into_iter().collect();
    /// let map_ref: &dyn Map = &map;
    ///
    /// assert_eq!(map_ref.get_as::<i32>(&"count"), Some(&42));
    /// assert_eq!(map_ref.get_as::<&str>(&"count"), None); // Wrong type
    /// assert_eq!(map_ref.get_as::<i32>(&"missing"), None); // Not found
    /// ```
    #[inline]
    pub fn get_as<T: Reflect>(&self, key: &dyn Reflect) -> Option<&T> {
        self.get(key).and_then(<dyn Reflect>::downcast_ref)
    }

    /// Returns a typed mutable reference to the value associated with the given key.
    ///
    /// Returns `None` if:
    /// - The key is not present in the map
    /// - The key type is incompatible with the map
    /// - The value cannot be downcast to type `T`
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::{Reflect, ops::Map};
    /// use std::collections::BTreeMap;
    ///
    /// let mut map: BTreeMap<&str, i32> = [("counter", 0)].into_iter().collect();
    /// let map_ref: &mut dyn Map = &mut map;
    ///
    /// if let Some(value) = map_ref.get_mut_as::<i32>(&"counter") {
    ///     *value += 1;
    /// }
    ///
    /// assert_eq!(map.get("counter"), Some(&1));
    /// ```
    #[inline]
    pub fn get_mut_as<T: Reflect>(&mut self, key: &dyn Reflect) -> Option<&mut T> {
        self.get_mut(key).and_then(<dyn Reflect>::downcast_mut)
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::DynamicMap;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(DynamicMap::type_path() == "voker_reflect::ops::DynamicMap");
        assert!(DynamicMap::module_path() == Some("voker_reflect::ops"));
        assert!(DynamicMap::type_ident() == "DynamicMap");
        assert!(DynamicMap::type_name() == "DynamicMap");
    }
}
