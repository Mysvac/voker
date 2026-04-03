use alloc::{boxed::Box, vec::Vec};
use core::cmp::Ordering;
use core::{fmt, ops::Deref};

use voker_utils::hash::{HashTable, hash_table};

use super::{impl_dynamic_type_info, impl_dynamic_type_path};
use crate::Reflect;
use crate::info::TypeInfo;
use crate::ops::{ApplyError, ReflectCloneError};
use crate::reflection::impl_reflect_cast_fn;

// -----------------------------------------------------------------------------
// Dynamic Set

/// A dynamic container representing a set-like collection.
///
/// `DynamicSet` is a type-erased dynamic set that can store values implementing [`Reflect`].
/// It mirrors associative collections such as [`BTreeSet`], enabling mutation and inspection
/// via reflection.
///
/// # Type Information
///
/// Dynamic types are special in that their `TypeInfo` is [`OpaqueInfo`](crate::info::OpaqueInfo),
/// but other APIs behave like the represented type, such as [`reflect_kind`] and [`reflect_ref`].
///
/// A `DynamicSet` can optionally represent a specific set type through its
/// [`represented_type_info`], allowing the dynamic set to be treated as if it were a specific
/// static set type for reflection purposes.
///
/// # Value Requirements
///
/// Values stored in a `DynamicSet` must support:
/// - Hashing via [`Reflect::reflect_hash`]
/// - Equality comparison via [`Reflect::reflect_eq`]
/// - Self-equality (a value must be equal to itself)
///
/// # Examples
///
/// ## Creating and populating a dynamic set
///
/// ```
/// use voker_reflect::ops::{DynamicSet, Set};
///
/// let mut set = DynamicSet::new();
/// set.extend("alpha");
/// set.extend("beta");
/// set.extend(42_i32);
///
/// assert_eq!(set.len(), 3);
/// assert!(set.contains(&"alpha"));
/// ```
///
/// ## Converting from a static set
///
/// ```
/// use std::collections::BTreeSet;
/// use voker_reflect::{Reflect, ops::Set};
///
/// let original: BTreeSet<&str> = ["red", "green", "blue"].into_iter().collect();
/// let dynamic = <dyn Set>::to_dynamic_set(&original);
///
/// assert_eq!(dynamic.len(), 3);
/// assert!(dynamic.contains(&"green"));
/// ```
///
/// [`reflect_kind`]: crate::Reflect::reflect_kind
/// [`reflect_ref`]: crate::Reflect::reflect_ref
/// [`represented_type_info`]: crate::Reflect::represented_type_info
/// [`BTreeSet`]: alloc::collections::BTreeSet
#[derive(Default)]
pub struct DynamicSet {
    info: Option<&'static TypeInfo>,
    hash_table: HashTable<Box<dyn Reflect>>,
}

impl_dynamic_type_path!(DynamicSet);
impl_dynamic_type_info!(DynamicSet);

impl DynamicSet {
    /// Creates an empty `DynamicSet`.
    ///
    /// This constructs a new, empty dynamic set. Use [`with_capacity`] when
    /// you know the approximate number of elements to avoid reallocations.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::DynamicSet;
    /// let set = DynamicSet::new();
    /// ```
    ///
    /// [`with_capacity`]: DynamicSet::with_capacity
    #[inline]
    pub const fn new() -> Self {
        Self {
            info: None,
            hash_table: HashTable::new(),
        }
    }

    /// Creates an empty `DynamicSet` with at least the given capacity.
    ///
    /// This reserves internal space to store `capacity` elements and can be used
    /// to reduce reallocations when inserting many values.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            info: None,
            hash_table: HashTable::with_capacity(capacity),
        }
    }

    /// Sets the [`TypeInfo`] that this dynamic set represents.
    ///
    /// When set, [`Reflect::represented_type_info`] will return this information,
    /// allowing the `DynamicSet` to be treated as if it were a specific static
    /// set type for reflection purposes.
    ///
    /// # Panics
    ///
    /// Panics if `info` is `Some` but does not contain set type information.
    #[inline]
    pub const fn set_type_info(&mut self, info: Option<&'static TypeInfo>) {
        match info {
            Some(info) => {
                assert!(info.kind().is_set(), "`TypeInfo` mismatched.");
                self.info = Some(info);
            }
            None => {
                self.info = None;
            }
        }
    }

    /// Inserts a boxed value into the set.
    ///
    /// This is the low-level insertion API that accepts an already-boxed
    /// `Reflect` value.
    ///
    /// Returns `true` if the value was newly inserted, and `false` if an
    /// equal value already existed (in which case the existing value is
    /// replaced by the provided one).
    ///
    /// # Panics
    ///
    /// Panics if the value does not support `reflect_hash` or
    /// `reflect_eq`, or if it is not equal to itself.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::DynamicSet;
    /// let mut set = DynamicSet::new();
    /// assert!(set.extend_boxed(Box::new("a")));
    /// assert!(!set.extend_boxed(Box::new("a")));
    /// ```
    pub fn extend_boxed(&mut self, value: Box<dyn Reflect>) -> bool {
        debug_assert_eq!(
            value.reflect_eq(&*value),
            Some(true),
            "The value is not `reflect_eq` to itself: `{}`.",
            value.reflect_type_path(),
        );

        match self
            .hash_table
            .find_mut(Self::internal_hash(&*value), Self::internal_eq(&*value))
        {
            Some(old) => {
                *old = value;
                false
            }
            None => {
                self.hash_table.insert_unique(
                    Self::internal_hash(value.as_ref()),
                    value,
                    |boxed| Self::internal_hash(boxed.as_ref()),
                );
                true
            }
        }
    }

    /// Inserts a value into the set by boxing it.
    ///
    /// Convenience wrapper around [`extend_boxed`] that takes a concrete
    /// `T: Reflect`, boxes it, and inserts it into the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_reflect::ops::DynamicSet;
    /// let mut set = DynamicSet::new();
    /// assert!(set.extend("hello"));
    /// assert!(!set.extend("hello"));
    /// ```
    ///
    /// [`extend_boxed`]: DynamicSet::extend_boxed
    #[inline]
    pub fn extend<T: Reflect>(&mut self, value: T) -> bool {
        self.extend_boxed(Box::new(value))
    }

    /// Compute the internal hash used by the set for a value.
    ///
    /// This calls `Reflect::reflect_hash` and panics if hashing is not
    /// supported by the value. The returned `u64` is used by the underlying
    /// hash table for indexing and lookups.
    fn internal_hash(value: &dyn Reflect) -> u64 {
        value.reflect_hash().unwrap_or_else(|| {
            panic!(
                "the given value of type `{}` does not support reflect hashing",
                value.reflect_type_path(),
            );
        })
    }

    /// Creates an equality predicate for comparing an owned boxed value with
    /// entries in the hash table.
    ///
    /// The returned closure compares `value` with `other` using
    /// `Reflect::reflect_eq`, and will panic if that operation is not
    /// supported on the compared value.
    fn internal_eq(value: &dyn Reflect) -> impl FnMut(&Box<dyn Reflect>) -> bool + '_ {
        |other| {
            value.reflect_eq(&**other).unwrap_or_else(|| {
                panic!(
                    "the given value of type `{}` does not support reflect hashing",
                    other.reflect_type_path(),
                )
            })
        }
    }
}

impl Reflect for DynamicSet {
    impl_reflect_cast_fn!(Set);

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
        Box::new(<Self as Set>::to_dynamic_set(self))
    }

    #[inline]
    fn reflect_clone(&self) -> Result<Box<dyn Reflect>, ReflectCloneError> {
        Ok(Box::new(<Self as Set>::to_dynamic_set(self)))
    }

    #[inline]
    fn apply(&mut self, value: &dyn Reflect) -> Result<(), ApplyError> {
        crate::impls::set_apply(self, value)
    }

    #[inline]
    fn reflect_hash(&self) -> Option<u64> {
        crate::impls::set_hash(self)
    }

    #[inline]
    fn reflect_eq(&self, other: &dyn Reflect) -> Option<bool> {
        crate::impls::set_eq(self, other)
    }

    #[inline]
    fn reflect_cmp(&self, other: &dyn Reflect) -> Option<Ordering> {
        crate::impls::set_cmp(self, other)
    }

    #[inline]
    fn reflect_debug(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DynamicSet(")?;
        crate::impls::set_debug(self, f)?;
        write!(f, ")")
    }
}

impl fmt::Debug for DynamicSet {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.reflect_debug(f)
    }
}

impl FromIterator<Box<dyn Reflect>> for DynamicSet {
    fn from_iter<I: IntoIterator<Item = Box<dyn Reflect>>>(values: I) -> Self {
        let mut this = DynamicSet::new();

        for value in values {
            this.insert(value);
        }

        this
    }
}

impl<T: Reflect> FromIterator<T> for DynamicSet {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        let mut this = DynamicSet::new();

        for value in values {
            this.insert(Box::new(value));
        }

        this
    }
}

impl IntoIterator for DynamicSet {
    type Item = Box<dyn Reflect>;
    type IntoIter = hash_table::IntoIter<Self::Item>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.hash_table.into_iter()
    }
}

impl<'a> IntoIterator for &'a DynamicSet {
    type Item = &'a dyn Reflect;
    type IntoIter = core::iter::Map<
        hash_table::Iter<'a, Box<dyn Reflect>>,
        fn(&'a Box<dyn Reflect>) -> Self::Item,
    >;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.hash_table.iter().map(Deref::deref)
    }
}

// -----------------------------------------------------------------------------
// Set Trait

/// A trait for type-erased set-like operations via reflection.
///
/// This trait represents any collection of unique values, including:
/// - B-tree sets (`BTreeSet<T>`)
/// - Hash-based sets (custom implementations)
/// - Other collections enforcing uniqueness
///
/// # Value Requirements
///
/// Implementors must ensure that elements:
/// 1. Support hashing via [`Reflect::reflect_hash`]
/// 2. Support equality comparison via [`Reflect::reflect_eq`]
/// 3. Are equal to themselves (reflexivity)
/// 4. Use consistent hashing (equal values have equal hashes)
///
/// The iteration order is not guaranteed by this trait and may vary by implementation.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeSet;
/// use voker_reflect::{Reflect, ops::Set};
///
/// let mut set: BTreeSet<&str> = ["oak", "elm"].into_iter().collect();
/// let set_ref: &mut dyn Set = &mut set;
///
/// assert_eq!(set_ref.len(), 2);
/// assert!(set_ref.contains(&"oak"));
/// assert!(!set_ref.contains(&"cedar"));
/// ```
pub trait Set: Reflect {
    /// Returns a reference to the value.
    ///
    /// If no value is contained, returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let set: BTreeSet<&str> = ["north", "south"].into_iter().collect();
    /// let set_ref: &dyn Set = &set;
    ///
    /// assert!(set_ref.get(&"north").is_some());
    /// assert!(set_ref.get(&"west").is_none());
    /// ```
    fn get(&self, value: &dyn Reflect) -> Option<&dyn Reflect>;

    /// Returns the number of elements in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let set: BTreeSet<i32> = [1, 2, 3].into_iter().collect();
    /// let set_ref: &dyn Set = &set;
    ///
    /// assert_eq!(set_ref.len(), 3);
    /// ```
    fn len(&self) -> usize;

    /// Returns `true` if the list contains no elements.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns an iterator over the values of the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let set: BTreeSet<i32> = [2, 4, 6].into_iter().collect();
    /// let set_ref: &dyn Set = &set;
    ///
    /// let sum: i32 = set_ref.iter()
    ///     .filter_map(|v| v.downcast_ref::<i32>())
    ///     .sum();
    ///
    /// assert_eq!(sum, 12);
    /// ```
    fn iter(&self) -> Box<dyn Iterator<Item = &dyn Reflect> + '_>;

    /// Drain the values of this set to get a vector of owned values.
    ///
    /// After calling this function, `self` will be empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let mut set: BTreeSet<i32> = [5, 7].into_iter().collect();
    /// let set_ref: &mut dyn Set = &mut set;
    ///
    /// let drained = set_ref.drain();
    /// assert_eq!(set_ref.len(), 0);
    /// assert_eq!(drained.len(), 2);
    /// ```
    fn drain(&mut self) -> Vec<Box<dyn Reflect>>;

    /// Retain only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` for which `f(&e)` returns `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let mut set: BTreeSet<i32> = [1, 2, 3, 4].into_iter().collect();
    /// let set_ref: &mut dyn Set = &mut set;
    ///
    /// set_ref.retain(&mut |value| {
    ///     value.downcast_ref::<i32>().map(|v| v % 2 == 0).unwrap_or(false)
    /// });
    ///
    /// assert_eq!(set_ref.len(), 2);
    /// assert!(set_ref.contains(&2));
    /// assert!(!set_ref.contains(&3));
    /// ```
    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect) -> bool);

    /// Creates a new [`DynamicSet`] from this set.
    ///
    /// Usually, `to_dynamic_map` recursively converts all data to a dynamic type,
    /// except for 'Opaque'. But for Set values, converting them to dynamic types
    /// is not a good idea, may cause changes in the result of hash and eq.
    ///
    /// Therefore,  we choose to directly clone them if feasible.
    fn to_dynamic_set(&self) -> DynamicSet {
        let mut set = DynamicSet::with_capacity(self.len());
        set.set_type_info(self.represented_type_info());
        for value in self.iter() {
            if let Ok(v) = value.reflect_clone() {
                debug_assert_eq!(
                    (*v).type_id(),
                    value.type_id(),
                    "`Reflect::reflect_clone` should return the same type: {}",
                    value.reflect_type_path(),
                );
                set.insert(v);
            } else {
                set.insert(value.to_dynamic());
            }
        }
        set
    }

    /// Inserts a value into the set.
    ///
    /// If the set had this value present, `true` is returned.
    /// If the set did not have this value present, `false` is returned.
    ///
    /// # Panics
    ///
    /// May panic if type incompatible.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let mut set: BTreeSet<&str> = BTreeSet::new();
    /// let set_ref: &mut dyn Set = &mut set;
    ///
    /// assert!(set_ref.insert(Box::new("apple")));
    /// assert!(!set_ref.insert(Box::new("apple")));
    /// ```
    fn insert(&mut self, value: Box<dyn Reflect>) -> bool;

    /// Try insert key values.
    ///
    /// If type incompatible, return `Err(V)`.
    ///
    /// Use for `` implementation, should not panic.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let mut set: BTreeSet<&str> = ["left"].into_iter().collect();
    /// let set_ref: &mut dyn Set = &mut set;
    ///
    /// let inserted = set_ref.try_insert(Box::new("right"))?;
    /// assert!(inserted);
    ///
    /// let duplicate = set_ref.try_insert(Box::new("left"))?;
    /// assert!(!duplicate);
    /// # Ok::<(), Box<dyn Reflect>>(())
    /// ```
    fn try_insert(&mut self, value: Box<dyn Reflect>) -> Result<bool, Box<dyn Reflect>>;

    /// Removes a value from the set.
    ///
    /// If the set did not have this value present, `true` is returned.
    /// If the set did have this value present, `false` is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let mut set: BTreeSet<i32> = [10, 20].into_iter().collect();
    /// let set_ref: &mut dyn Set = &mut set;
    ///
    /// assert!(set_ref.remove(&10));
    /// assert!(!set_ref.remove(&30));
    /// ```
    fn remove(&mut self, value: &dyn Reflect) -> bool;

    /// Checks if the given value is contained in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let set: BTreeSet<&str> = ["mercury", "venus"].into_iter().collect();
    /// let set_ref: &dyn Set = &set;
    ///
    /// assert!(set_ref.contains(&"mercury"));
    /// assert!(!set_ref.contains(&"earth"));
    /// ```
    fn contains(&self, value: &dyn Reflect) -> bool;
}

impl Set for DynamicSet {
    #[inline]
    fn get(&self, value: &dyn Reflect) -> Option<&dyn Reflect> {
        self.hash_table
            .find(Self::internal_hash(value), Self::internal_eq(value))
            .map(Deref::deref)
    }

    #[inline]
    fn len(&self) -> usize {
        self.hash_table.len()
    }

    #[inline]
    fn iter(&self) -> Box<dyn Iterator<Item = &dyn Reflect> + '_> {
        Box::new(self.hash_table.iter().map(Deref::deref))
    }

    #[inline]
    fn drain(&mut self) -> Vec<Box<dyn Reflect>> {
        self.hash_table.drain().collect::<Vec<_>>()
    }

    #[inline]
    fn retain(&mut self, f: &mut dyn FnMut(&dyn Reflect) -> bool) {
        self.hash_table.retain(move |value| f(&**value));
    }

    fn insert(&mut self, value: Box<dyn Reflect>) -> bool {
        self.extend_boxed(value)
    }

    #[inline]
    fn try_insert(&mut self, value: Box<dyn Reflect>) -> Result<bool, Box<dyn Reflect>> {
        Ok(self.extend_boxed(value))
    }

    #[inline]
    fn remove(&mut self, value: &dyn Reflect) -> bool {
        self.hash_table
            .find_entry(Self::internal_hash(value), Self::internal_eq(value))
            .map(hash_table::OccupiedEntry::remove)
            .is_ok()
    }

    #[inline]
    fn contains(&self, value: &dyn Reflect) -> bool {
        self.hash_table
            .find(Self::internal_hash(value), Self::internal_eq(value))
            .is_some()
    }
}

impl dyn Set {
    /// Returns a typed reference to the value matching `key`.
    ///
    /// This is a convenience helper that calls `get` and attempts to downcast
    /// the resulting `&dyn Reflect` to `&T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::BTreeSet;
    /// use voker_reflect::{Reflect, ops::Set};
    ///
    /// let set: BTreeSet<i32> = [1, 2, 3].into_iter().collect();
    /// let set_ref: &dyn Set = &set;
    ///
    /// assert_eq!(set_ref.get_as::<i32>(&1), Some(&1));
    /// assert_eq!(set_ref.get_as::<i32>(&4), None);
    /// ```
    #[inline]
    pub fn get_as<T: Reflect>(&self, key: &dyn Reflect) -> Option<&T> {
        self.get(key).and_then(<dyn Reflect>::downcast_ref)
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::DynamicSet;
    use crate::info::TypePath;

    #[test]
    fn type_path() {
        assert!(DynamicSet::type_path() == "voker_reflect::ops::DynamicSet");
        assert!(DynamicSet::module_path() == Some("voker_reflect::ops"));
        assert!(DynamicSet::type_ident() == "DynamicSet");
        assert!(DynamicSet::type_name() == "DynamicSet");
    }
}
