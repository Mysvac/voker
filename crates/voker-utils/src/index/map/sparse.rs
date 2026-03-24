//! Provide [`SparseIndexMap`] based on [indexmap]'s implementation.

use core::cmp::Ordering;
use core::fmt::Debug;
use core::hash::Hash;
use core::ops::{Index, IndexMut, RangeBounds};

use alloc::boxed::Box;
use indexmap::map::{Drain, ExtractIf, IntoIter, IntoKeys, IntoValues};
use indexmap::map::{Entry, IndexedEntry, Slice};
use indexmap::map::{Iter, IterMut, Keys, Splice, Values, ValuesMut};
use indexmap::{Equivalent, GetDisjointMutError, TryReserveError};

use crate::hash::SparseHashState;

type InternalMap<K, V> = indexmap::IndexMap<K, V, SparseHashState>;

// -----------------------------------------------------------------------------
// SparseIndexMap

/// New-type for [`indexmap::IndexMap`] with [`SparseHashState`] as the default hashing provider.
///
/// Can be trivially converted to and from a [`indexmap::IndexMap`] using [`From`].
///
/// This provides determinism by default with an acceptable compromise to denial
/// of service resistance in the context of a game engine.
///
/// # Examples
///
/// ```
/// use voker_utils::index::SparseIndexMap;
///
/// let mut scores = SparseIndexMap::new();
///
/// scores.insert("a", 25);
/// scores.insert("b", 24);
/// scores.insert("c", 12);
///
/// for (name, score) in &scores {
///     // Fixed printing order,
///     // Must be a -> b -> c .
///     println!("{}: {}", name, score);
/// }
/// ```
#[repr(transparent)]
pub struct SparseIndexMap<K, V>(InternalMap<K, V>);

// -----------------------------------------------------------------------------
// `SparseHashState` specific methods

impl<K: Eq + Hash, V, const N: usize> From<[(K, V); N]> for SparseIndexMap<K, V> {
    fn from(value: [(K, V); N]) -> Self {
        value.into_iter().collect()
    }
}

impl<K, V> SparseIndexMap<K, V> {
    /// Create a empty [`SparseIndexMap`]
    ///
    /// # Example
    ///
    /// ```rust
    /// use voker_utils::index::SparseIndexMap;
    ///
    /// let map = SparseIndexMap::new();
    /// # // docs test
    /// # let mut map = map;
    /// # map.insert(0usize, "foo");
    /// # assert_eq!(map.get(&0), Some("foo").as_ref());
    /// ```
    #[inline(always)]
    pub const fn new() -> Self {
        Self(InternalMap::with_hasher(SparseHashState))
    }

    /// Create a empty [`SparseIndexMap`] with specific capacity
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// #
    /// let map = SparseIndexMap::with_capacity(5);
    /// # // docs test
    /// # let mut map = map;
    /// # map.insert(0usize, "foo");
    /// # assert_eq!(map.get(&0), Some("foo").as_ref());
    /// ```
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(InternalMap::with_capacity_and_hasher(
            capacity,
            SparseHashState,
        ))
    }
}

// -----------------------------------------------------------------------------
// Transmute

// impl<K, V> Deref for SparseIndexMap<K, V> {
//     type Target = InternalMap<K, V>;
//
//     #[inline(always)]
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl<K, V> DerefMut for SparseIndexMap<K, V> {
//     #[inline(always)]
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0
//     }
// }

// -----------------------------------------------------------------------------
// Re-export the underlying method

impl<K, V> Clone for SparseIndexMap<K, V>
where
    InternalMap<K, V>: Clone,
{
    #[inline(always)]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0);
    }
}

impl<K, V> Debug for SparseIndexMap<K, V>
where
    InternalMap<K, V>: Debug,
{
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <InternalMap<K, V> as Debug>::fmt(&self.0, f)
    }
}

impl<K, V> Default for SparseIndexMap<K, V>
where
    InternalMap<K, V>: Default,
{
    #[inline(always)]
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K, V> PartialEq for SparseIndexMap<K, V>
where
    InternalMap<K, V>: PartialEq,
{
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<K, V> Eq for SparseIndexMap<K, V> where InternalMap<K, V>: Eq {}

impl<K, V, T> FromIterator<T> for SparseIndexMap<K, V>
where
    InternalMap<K, V>: FromIterator<T>,
{
    #[inline(always)]
    fn from_iter<U: IntoIterator<Item = T>>(iter: U) -> Self {
        Self(FromIterator::from_iter(iter))
    }
}

impl<K, V, T> Index<T> for SparseIndexMap<K, V>
where
    InternalMap<K, V>: Index<T>,
{
    type Output = <InternalMap<K, V> as Index<T>>::Output;

    #[inline(always)]
    fn index(&self, index: T) -> &Self::Output {
        self.0.index(index)
    }
}

impl<K, V, T> IndexMut<T> for SparseIndexMap<K, V>
where
    InternalMap<K, V>: IndexMut<T>,
{
    #[inline(always)]
    fn index_mut(&mut self, index: T) -> &mut Self::Output {
        self.0.index_mut(index)
    }
}

impl<K, V> IntoIterator for SparseIndexMap<K, V>
where
    InternalMap<K, V>: IntoIterator,
{
    type Item = <InternalMap<K, V> as IntoIterator>::Item;
    type IntoIter = <InternalMap<K, V> as IntoIterator>::IntoIter;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a SparseIndexMap<K, V>
where
    &'a InternalMap<K, V>: IntoIterator,
{
    type Item = <&'a InternalMap<K, V> as IntoIterator>::Item;
    type IntoIter = <&'a InternalMap<K, V> as IntoIterator>::IntoIter;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut SparseIndexMap<K, V>
where
    &'a mut InternalMap<K, V>: IntoIterator,
{
    type Item = <&'a mut InternalMap<K, V> as IntoIterator>::Item;
    type IntoIter = <&'a mut InternalMap<K, V> as IntoIterator>::IntoIter;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        (&mut self.0).into_iter()
    }
}

impl<K, V, T> Extend<T> for SparseIndexMap<K, V>
where
    InternalMap<K, V>: Extend<T>,
{
    #[inline(always)]
    fn extend<U: IntoIterator<Item = T>>(&mut self, iter: U) {
        self.0.extend(iter);
    }
}

impl<K, V> serde_core::Serialize for SparseIndexMap<K, V>
where
    InternalMap<K, V>: serde_core::Serialize,
{
    #[inline(always)]
    fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
    where
        T: serde_core::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, K, V> serde_core::Deserialize<'de> for SparseIndexMap<K, V>
where
    InternalMap<K, V>: serde_core::Deserialize<'de>,
{
    #[inline(always)]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde_core::Deserializer<'de>,
    {
        Ok(Self(serde_core::Deserialize::deserialize(deserializer)?))
    }
}

impl<K, V> SparseIndexMap<K, V> {
    /// Returns the number of elements the map can hold without reallocating.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// let map = SparseIndexMap::with_capacity(5);
    ///
    /// # // doc test
    /// # let map: SparseIndexMap<(), ()> = map;
    /// # assert!(map.capacity() >= 5);
    /// ```
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// An iterator visiting all keys in arbitrary order.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// #
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// for key in map.keys() {
    ///     // foo, bar, baz (arbitrary order)
    /// }
    /// # assert_eq!(map.keys().count(), 3);
    /// ```
    #[inline(always)]
    pub fn keys(&self) -> Keys<'_, K, V> {
        self.0.keys()
    }

    /// An iterator visiting all values in arbitrary order.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// #
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// for key in map.values() {
    ///     // 0, 1, 2 (arbitrary order)
    /// }
    /// # assert_eq!(map.values().count(), 3);
    /// ```
    #[inline(always)]
    pub fn values(&self) -> Values<'_, K, V> {
        self.0.values()
    }

    /// An iterator visiting all values mutably in arbitrary order.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// #
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// for key in map.values_mut() {
    ///     // 0, 1, 2 (arbitrary order)
    /// }
    /// # assert_eq!(map.values_mut().count(), 3);
    /// ```
    #[inline(always)]
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        self.0.values_mut()
    }

    /// An iterator visiting all key-value pairs in arbitrary order.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// #
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// for (key, value) in map.iter() {
    ///     // ("foo", 0), ("bar", 1), ("baz", 2) (arbitrary order)
    /// }
    /// # assert_eq!(map.iter().count(), 3);
    /// ```
    #[inline(always)]
    pub fn iter(&self) -> Iter<'_, K, V> {
        self.0.iter()
    }

    /// An iterator visiting all key-value pairs in arbitrary order, with mutable references to the values.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// #
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// for (key, value) in map.iter_mut() {
    ///     // ("foo", 0), ("bar", 1), ("baz", 2) (arbitrary order)
    /// }
    /// # assert_eq!(map.iter_mut().count(), 3);
    /// ```
    #[inline(always)]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        self.0.iter_mut()
    }

    /// Returns the number of elements in the map.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// let mut map = SparseIndexMap::new();
    ///
    /// assert_eq!(map.len(), 0);
    ///
    /// map.insert("foo", 0);
    ///
    /// assert_eq!(map.len(), 1);
    /// ```
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the map contains no elements.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// let mut map = SparseIndexMap::new();
    ///
    /// assert!(map.is_empty());
    ///
    /// map.insert("foo", 0);
    ///
    /// assert!(!map.is_empty());
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Clears the map, returning all key-value pairs as an iterator. Keeps the allocated memory for reuse.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// #
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// for (key, value) in map.drain(..) {
    ///     // ("foo", 0), ("bar", 1), ("baz", 2) (fixed order)
    /// }
    ///
    /// assert!(map.is_empty());
    /// ```
    #[inline(always)]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, K, V>
    where
        R: RangeBounds<usize>,
    {
        self.0.drain(range)
    }

    /// Creates an iterator which uses a closure to determine if an element should be removed,
    /// for all elements in the given range.
    ///
    /// If the closure returns true, the element is removed from the map and yielded.
    /// If the closure returns false, or panics, the element remains in the map and will not be
    /// yielded.
    ///
    /// Note that `extract_if` lets you mutate every value in the filter closure, regardless of
    /// whether you choose to keep or remove it.
    ///
    /// The range may be any type that implements [`RangeBounds<usize>`],
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To check the entire map, use `RangeFull`
    /// like `map.extract_if(.., predicate)`.
    ///
    /// If the returned `ExtractIf` is not exhausted, e.g. because it is dropped without iterating
    /// or the iteration short-circuits, then the remaining elements will be retained.
    /// Use [`retain`] with a negated predicate if you do not need the returned iterator.
    ///
    /// [`retain`]: SparseIndexMap::retain
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the map.
    ///
    /// # Examples
    ///
    /// Splitting a map into even and odd keys, reusing the original map:
    ///
    /// ```
    /// use voker_utils::index::SparseIndexMap;
    ///
    /// let mut map: SparseIndexMap<i32, i32> = (0..8).map(|x| (x, x)).collect();
    /// let extracted: SparseIndexMap<i32, i32> = map.extract_if(.., |k, _v| k % 2 == 0).collect();
    ///
    /// let evens = extracted.keys().copied().collect::<Vec<_>>();
    /// let odds = map.keys().copied().collect::<Vec<_>>();
    ///
    /// assert_eq!(evens, vec![0, 2, 4, 6]);
    /// assert_eq!(odds, vec![1, 3, 5, 7]);
    /// ```
    #[inline(always)]
    pub fn extract_if<F, R>(&mut self, range: R, pred: F) -> ExtractIf<'_, K, V, F>
    where
        F: FnMut(&K, &mut V) -> bool,
        R: RangeBounds<usize>,
    {
        self.0.extract_if(range, pred)
    }

    /// Clears the map, removing all key-value pairs. Keeps the allocated memory for reuse.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// map.clear();
    ///
    /// assert!(map.is_empty());
    /// ```
    #[inline(always)]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Creates a consuming iterator visiting all the keys in arbitrary order.
    /// The map cannot be used after calling this. The iterator element type is K.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// for key in map.into_keys() {
    ///     // "foo", "bar", "baz" (arbitrary order)
    /// }
    /// ```
    #[inline(always)]
    pub fn into_keys(self) -> IntoKeys<K, V> {
        self.0.into_keys()
    }

    /// Creates a consuming iterator visiting all the values in arbitrary order.
    /// The map cannot be used after calling this. The iterator element type is V.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexMap;
    /// #
    /// let mut map = SparseIndexMap::new();
    ///
    /// map.insert("foo", 0);
    /// map.insert("bar", 1);
    /// map.insert("baz", 2);
    ///
    /// for key in map.into_values() {
    ///     // 0, 1, 2 (arbitrary order)
    /// }
    /// ```
    #[inline(always)]
    pub fn into_values(self) -> IntoValues<K, V> {
        self.0.into_values()
    }

    /// Shortens the map, keeping the first `len` elements and dropping the rest.
    #[inline(always)]
    pub fn truncate(&mut self, len: usize) {
        self.0.truncate(len);
    }

    /// Splits the collection into two at the given index.
    #[inline(always)]
    pub fn split_off(&mut self, at: usize) -> Self {
        Self(self.0.split_off(at))
    }

    /// Reserve capacity for `additional` more key-value pairs.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional)
    }

    /// Reserve capacity for `additional` more key-value pairs, without over-allocating.
    ///
    /// Unlike `reserve`, this does not deliberately over-allocate the entry capacity to avoid
    /// frequent re-allocations. However, the underlying data structures may still have internal
    /// capacity requirements, and the allocator itself may give more space than requested, so this
    /// cannot be relied upon to be precisely minimal.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.0.reserve_exact(additional)
    }

    /// Try to reserve capacity for `additional` more key-value pairs.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve(additional)
    }

    /// Try to reserve capacity for `additional` more key-value pairs, without over-allocating.
    ///
    /// Unlike `try_reserve`, this does not deliberately over-allocate the entry capacity to avoid
    /// frequent re-allocations. However, the underlying data structures may still have internal
    /// capacity requirements, and the allocator itself may give more space than requested, so this
    /// cannot be relied upon to be precisely minimal.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve_exact(additional)
    }

    /// Shrink the capacity of the map as much as possible.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }

    /// Shrink the capacity of the map with a lower limit.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.0.shrink_to(min_capacity);
    }
}

impl<K, V> SparseIndexMap<K, V>
where
    K: Hash + Eq,
{
    /// Insert a key-value pair in the map.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// retains in its place in the order, its corresponding value is updated
    /// with `value`, and the older value is returned inside `Some(_)`.
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted, last in order, and `None` is returned.
    ///
    /// Computes in **O(1)** time (amortized average).
    ///
    /// See also [`entry`][Self::entry] if you want to insert *or* modify,
    /// or [`insert_full`][Self::insert_full] if you need to get the index of
    /// the corresponding key-value pair.
    #[inline(always)]
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.0.insert(key, value)
    }

    /// Insert a key-value pair in the map, and get their index.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// retains in its place in the order, its corresponding value is updated
    /// with `value`, and the older value is returned inside `(index, Some(_))`.
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted, last in order, and `(index, None)` is returned.
    ///
    /// Computes in **O(1)** time (amortized average).
    ///
    /// See also [`entry`][Self::entry] if you want to insert *or* modify.
    #[inline(always)]
    pub fn insert_full(&mut self, key: K, value: V) -> (usize, Option<V>) {
        self.0.insert_full(key, value)
    }

    /// Insert a key-value pair in the map at its ordered position among sorted keys.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_keys`][Self::binary_search_keys], then either updating
    /// it or calling [`insert_before`][Self::insert_before] for a new key.
    ///
    /// If the sorted key is found in the map, its corresponding value is
    /// updated with `value`, and the older value is returned inside
    /// `(index, Some(_))`. Otherwise, the new key-value pair is inserted at
    /// the sorted position, and `(index, None)` is returned.
    ///
    /// If the existing keys are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the key-value
    /// pair is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average). Instead of repeating calls to
    /// `insert_sorted`, it may be faster to call batched [`insert`][Self::insert]
    /// or [`extend`][Self::extend] and only call [`sort_keys`][Self::sort_keys]
    /// or [`sort_unstable_keys`][Self::sort_unstable_keys] once.
    #[inline(always)]
    pub fn insert_sorted(&mut self, key: K, value: V) -> (usize, Option<V>)
    where
        K: Ord,
    {
        self.0.insert_sorted(key, value)
    }

    /// Insert a key-value pair in the map at its ordered position among keys
    /// sorted by `cmp`.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_by`][Self::binary_search_by], then calling
    /// [`insert_before`][Self::insert_before] with the given key and value.
    ///
    /// If the existing keys are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the key-value
    /// pair is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn insert_sorted_by<F>(&mut self, key: K, value: V, cmp: F) -> (usize, Option<V>)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.0.insert_sorted_by(key, value, cmp)
    }

    /// Insert a key-value pair in the map at its ordered position
    /// using a sort-key extraction function.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_by_key`][Self::binary_search_by_key] with `sort_key(key)`, then
    /// calling [`insert_before`][Self::insert_before] with the given key and value.
    ///
    /// If the existing keys are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the key-value
    /// pair is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn insert_sorted_by_key<B, F>(
        &mut self,
        key: K,
        value: V,
        sort_key: F,
    ) -> (usize, Option<V>)
    where
        B: Ord,
        F: FnMut(&K, &V) -> B,
    {
        self.0.insert_sorted_by_key(key, value, sort_key)
    }

    /// Insert a key-value pair in the map before the entry at the given index, or at the end.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// is moved to the new position in the map, its corresponding value is updated
    /// with `value`, and the older value is returned inside `Some(_)`. The returned index
    /// will either be the given index or one less, depending on how the entry moved.
    /// (See [`shift_insert`](Self::shift_insert) for different behavior here.)
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted exactly at the given index, and `None` is returned.
    ///
    /// ***Panics*** if `index` is out of bounds.
    /// Valid indices are `0..=map.len()` (inclusive).
    ///
    /// Computes in **O(n)** time (average).
    ///
    /// See also [`entry`][Self::entry] if you want to insert *or* modify,
    /// perhaps only using the index for new entries with `VacantEntry::shift_insert`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexMap;
    /// let mut map: SparseIndexMap<char, ()> = ('a'..='z').map(|c| (c, ())).collect();
    ///
    /// // The new key '*' goes exactly at the given index.
    /// assert_eq!(map.get_index_of(&'*'), None);
    /// assert_eq!(map.insert_before(10, '*', ()), (10, None));
    /// assert_eq!(map.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the key 'a' up will shift others down, so this moves *before* 10 to index 9.
    /// assert_eq!(map.insert_before(10, 'a', ()), (9, Some(())));
    /// assert_eq!(map.get_index_of(&'a'), Some(9));
    /// assert_eq!(map.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the key 'z' down will shift others up, so this moves to exactly 10.
    /// assert_eq!(map.insert_before(10, 'z', ()), (10, Some(())));
    /// assert_eq!(map.get_index_of(&'z'), Some(10));
    /// assert_eq!(map.get_index_of(&'*'), Some(11));
    ///
    /// // Moving or inserting before the endpoint is also valid.
    /// assert_eq!(map.len(), 27);
    /// assert_eq!(map.insert_before(map.len(), '*', ()), (26, Some(())));
    /// assert_eq!(map.get_index_of(&'*'), Some(26));
    /// assert_eq!(map.insert_before(map.len(), '+', ()), (27, None));
    /// assert_eq!(map.get_index_of(&'+'), Some(27));
    /// assert_eq!(map.len(), 28);
    /// ```
    #[inline(always)]
    pub fn insert_before(&mut self, index: usize, key: K, value: V) -> (usize, Option<V>) {
        self.0.insert_before(index, key, value)
    }

    /// Insert a key-value pair in the map at the given index.
    ///
    /// If an equivalent key already exists in the map: the key remains and
    /// is moved to the given index in the map, its corresponding value is updated
    /// with `value`, and the older value is returned inside `Some(_)`.
    /// Note that existing entries **cannot** be moved to `index == map.len()`!
    /// (See [`insert_before`](Self::insert_before) for different behavior here.)
    ///
    /// If no equivalent key existed in the map: the new key-value pair is
    /// inserted at the given index, and `None` is returned.
    ///
    /// ***Panics*** if `index` is out of bounds.
    /// Valid indices are `0..map.len()` (exclusive) when moving an existing entry, or
    /// `0..=map.len()` (inclusive) when inserting a new key.
    ///
    /// Computes in **O(n)** time (average).
    ///
    /// See also [`entry`][Self::entry] if you want to insert *or* modify,
    /// perhaps only using the index for new entries with `VacantEntry::shift_insert`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexMap;
    /// let mut map: SparseIndexMap<char, ()> = ('a'..='z').map(|c| (c, ())).collect();
    ///
    /// // The new key '*' goes exactly at the given index.
    /// assert_eq!(map.get_index_of(&'*'), None);
    /// assert_eq!(map.shift_insert(10, '*', ()), None);
    /// assert_eq!(map.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the key 'a' up to 10 will shift others down, including the '*' that was at 10.
    /// assert_eq!(map.shift_insert(10, 'a', ()), Some(()));
    /// assert_eq!(map.get_index_of(&'a'), Some(10));
    /// assert_eq!(map.get_index_of(&'*'), Some(9));
    ///
    /// // Moving the key 'z' down to 9 will shift others up, including the '*' that was at 9.
    /// assert_eq!(map.shift_insert(9, 'z', ()), Some(()));
    /// assert_eq!(map.get_index_of(&'z'), Some(9));
    /// assert_eq!(map.get_index_of(&'*'), Some(10));
    ///
    /// // Existing keys can move to len-1 at most, but new keys can insert at the endpoint.
    /// assert_eq!(map.len(), 27);
    /// assert_eq!(map.shift_insert(map.len() - 1, '*', ()), Some(()));
    /// assert_eq!(map.get_index_of(&'*'), Some(26));
    /// assert_eq!(map.shift_insert(map.len(), '+', ()), None);
    /// assert_eq!(map.get_index_of(&'+'), Some(27));
    /// assert_eq!(map.len(), 28);
    /// ```
    #[inline(always)]
    pub fn shift_insert(&mut self, index: usize, key: K, value: V) -> Option<V> {
        self.0.shift_insert(index, key, value)
    }

    /// Replaces the key at the given index. The new key does not need to be
    /// equivalent to the one it is replacing, but it must be unique to the rest
    /// of the map.
    ///
    /// Returns `Ok(old_key)` if successful, or `Err((other_index, key))` if an
    /// equivalent key already exists at a different index. The map will be
    /// unchanged in the error case.
    ///
    /// Direct indexing can be used to change the corresponding value: simply
    /// `map[index] = value`, or `mem::replace(&mut map[index], value)` to
    /// retrieve the old value as well.
    ///
    /// ***Panics*** if `index` is out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn replace_index(&mut self, index: usize, key: K) -> Result<K, (usize, K)> {
        self.0.replace_index(index, key)
    }

    /// Get the given key's corresponding entry in the map for insertion and/or
    /// in-place manipulation.
    ///
    /// Computes in **O(1)** time (amortized average).
    #[inline(always)]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        self.0.entry(key)
    }

    /// Creates a splicing iterator that replaces the specified range in the map
    /// with the given `replace_with` key-value iterator and yields the removed
    /// items. `replace_with` does not need to be the same length as `range`.
    ///
    /// The `range` is removed even if the iterator is not consumed until the
    /// end. It is unspecified how many elements are removed from the map if the
    /// `Splice` value is leaked.
    ///
    /// The input iterator `replace_with` is only consumed when the `Splice`
    /// value is dropped. If a key from the iterator matches an existing entry
    /// in the map (outside of `range`), then the value will be updated in that
    /// position. Otherwise, the new key-value pair will be inserted in the
    /// replaced `range`.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexMap;
    ///
    /// let mut map = SparseIndexMap::from([(0, '_'), (1, 'a'), (2, 'b'), (3, 'c'), (4, 'd')]);
    /// let new = [(5, 'E'), (4, 'D'), (3, 'C'), (2, 'B'), (1, 'A')];
    /// let removed: Vec<_> = map.splice(2..4, new).collect();
    ///
    /// // 1 and 4 got new values, while 5, 3, and 2 were newly inserted.
    /// assert!(map.into_iter().eq([(0, '_'), (1, 'A'), (5, 'E'), (3, 'C'), (2, 'B'), (4, 'D')]));
    /// assert_eq!(removed, &[(2, 'b'), (3, 'c')]);
    /// ```
    #[inline(always)]
    pub fn splice<R, I>(
        &mut self,
        range: R,
        replace_with: I,
    ) -> Splice<'_, I::IntoIter, K, V, SparseHashState>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = (K, V)>,
    {
        self.0.splice(range, replace_with)
    }

    /// Moves all key-value pairs from `other` into `self`, leaving `other` empty.
    ///
    /// This is equivalent to calling [`insert`][Self::insert] for each
    /// key-value pair from `other` in order, which means that for keys that
    /// already exist in `self`, their value is updated in the current position.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexMap;
    ///
    /// // Note: Key (3) is present in both maps.
    /// let mut a = SparseIndexMap::from([(3, "c"), (2, "b"), (1, "a")]);
    /// let mut b = SparseIndexMap::from([(3, "d"), (4, "e"), (5, "f")]);
    /// let old_capacity = b.capacity();
    ///
    /// a.append(&mut b);
    ///
    /// assert_eq!(a.len(), 5);
    /// assert_eq!(b.len(), 0);
    /// assert_eq!(b.capacity(), old_capacity);
    ///
    /// assert!(a.keys().eq(&[3, 2, 1, 4, 5]));
    /// assert_eq!(a[&3], "d"); // "c" was overwritten.
    /// ```
    #[inline(always)]
    pub fn append(&mut self, other: &mut SparseIndexMap<K, V>) {
        self.0.append(&mut other.0);
    }
}

impl<K, V> SparseIndexMap<K, V> {
    /// Return `true` if an equivalent to `key` exists in the map.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.contains_key(key)
    }

    /// Return a reference to the stored value for `key`, if it is present,
    /// else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get(key)
    }

    /// Return references to the stored key-value pair for the lookup `key`,
    /// if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get_key_value<Q>(&self, key: &Q) -> Option<(&K, &V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get_key_value(key)
    }

    /// Return the index with references to the stored key-value pair for the
    /// lookup `key`, if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get_full<Q>(&self, key: &Q) -> Option<(usize, &K, &V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get_full(key)
    }

    /// Return the item index for `key`, if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get_index_of<Q>(&self, key: &Q) -> Option<usize>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get_index_of(key)
    }

    /// Return a mutable reference to the stored value for `key`,
    /// if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get_mut(key)
    }

    /// Return a reference and mutable references to the stored key-value pair
    /// for the lookup `key`, if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get_key_value_mut<Q>(&mut self, key: &Q) -> Option<(&K, &mut V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get_key_value_mut(key)
    }

    /// Return the index with a reference and mutable reference to the stored
    /// key-value pair for the lookup `key`, if it is present, else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get_full_mut<Q>(&mut self, key: &Q) -> Option<(usize, &K, &mut V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get_full_mut(key)
    }

    /// Return the values for `N` keys. If any key is duplicated, this function will panic.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::index::SparseIndexMap;
    /// let mut map = SparseIndexMap::from([(1, 'a'), (3, 'b'), (2, 'c')]);
    /// assert_eq!(map.get_disjoint_mut([&2, &1]), [Some(&mut 'c'), Some(&mut 'a')]);
    /// ```
    #[inline(always)]
    pub fn get_disjoint_mut<Q, const N: usize>(&mut self, keys: [&Q; N]) -> [Option<&mut V>; N]
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.get_disjoint_mut(keys)
    }

    /// Remove the key-value pair equivalent to `key` and return
    /// its value.
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.swap_remove(key)
    }

    /// Remove and return the key-value pair equivalent to `key`.
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.shift_remove_entry(key)
    }

    /// Remove the key-value pair equivalent to `key` and return it and
    /// the index it had.
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_remove_full<Q>(&mut self, key: &Q) -> Option<(usize, K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.shift_remove_full(key)
    }

    /// Remove the key-value pair equivalent to `key` and return
    /// its value.
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn shift_remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.shift_remove(key)
    }

    /// Remove and return the key-value pair equivalent to `key`.
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn shift_remove_entry<Q>(&mut self, key: &Q) -> Option<(K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.shift_remove_entry(key)
    }

    /// Remove the key-value pair equivalent to `key` and return it and
    /// the index it had.
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `key` is not in map.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn shift_remove_full<Q>(&mut self, key: &Q) -> Option<(usize, K, V)>
    where
        Q: ?Sized + Hash + Equivalent<K>,
    {
        self.0.shift_remove_full(key)
    }
}

impl<K, V> SparseIndexMap<K, V> {
    /// Remove the last key-value pair
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    #[doc(alias = "pop_last")] // like `BTreeMap`
    #[inline(always)]
    pub fn pop(&mut self) -> Option<(K, V)> {
        self.0.pop()
    }

    /// Removes and returns the last key-value pair from a map if the predicate
    /// returns `true`, or [`None`] if the predicate returns false or the map
    /// is empty (the predicate will not be called in that case).
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexMap;
    ///
    /// let init = [(1, 'a'), (2, 'b'), (3, 'c'), (4, 'd')];
    /// let mut map = SparseIndexMap::from(init);
    /// let pred = |key: &i32, _value: &mut char| *key % 2 == 0;
    ///
    /// assert_eq!(map.pop_if(pred), Some((4, 'd')));
    /// assert_eq!(map.as_slice(), &init[..3]);
    /// assert_eq!(map.pop_if(pred), None);
    /// ```
    #[inline(always)]
    pub fn pop_if(&mut self, predicate: impl FnOnce(&K, &mut V) -> bool) -> Option<(K, V)> {
        self.0.pop_if(predicate)
    }

    /// Scan through each key-value pair in the map and keep those where the
    /// closure `keep` returns `true`.
    ///
    /// The elements are visited in order, and remaining elements keep their
    /// order.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn retain<F>(&mut self, keep: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        self.0.retain(keep);
    }

    /// Sort the map's key-value pairs by the default ordering of the keys.
    ///
    /// This is a stable sort -- but equivalent keys should not normally coexist in
    /// a map at all, so [`sort_unstable_keys`][Self::sort_unstable_keys] is preferred
    /// because it is generally faster and doesn't allocate auxiliary memory.
    ///
    /// See [`sort_by`](Self::sort_by) for details.
    #[inline(always)]
    pub fn sort_keys(&mut self)
    where
        K: Ord,
    {
        self.0.sort_keys();
    }

    /// Sort the map's key-value pairs in place using the comparison
    /// function `cmp`.
    ///
    /// The comparison function receives two key and value pairs to compare (you
    /// can sort by keys or values or their combination as needed).
    ///
    /// Computes in **O(n log n + c)** time and **O(n)** space where *n* is
    /// the length of the map and *c* the capacity. The sort is stable.
    #[inline(always)]
    pub fn sort_by<F>(&mut self, cmp: F)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.0.sort_by(cmp);
    }

    /// Sort the key-value pairs of the map and return a by-value iterator of
    /// the key-value pairs with the result.
    ///
    /// The sort is stable.
    #[inline(always)]
    pub fn sorted_by<F>(self, cmp: F) -> IntoIter<K, V>
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.0.sorted_by(cmp)
    }

    /// Sort the map's key-value pairs in place using a sort-key extraction function.
    ///
    /// Computes in **O(n log n + c)** time and **O(n)** space where *n* is
    /// the length of the map and *c* the capacity. The sort is stable.
    #[inline(always)]
    pub fn sort_by_key<T, F>(&mut self, sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.0.sort_by_key(sort_key);
    }

    /// Sort the map's key-value pairs by the default ordering of the keys, but
    /// may not preserve the order of equal elements.
    ///
    /// See [`sort_unstable_by`](Self::sort_unstable_by) for details.
    #[inline(always)]
    pub fn sort_unstable_keys(&mut self)
    where
        K: Ord,
    {
        self.0.sort_unstable_keys();
    }

    /// Sort the map's key-value pairs in place using the comparison function `cmp`, but
    /// may not preserve the order of equal elements.
    ///
    /// The comparison function receives two key and value pairs to compare (you
    /// can sort by keys or values or their combination as needed).
    ///
    /// Computes in **O(n log n + c)** time where *n* is
    /// the length of the map and *c* is the capacity. The sort is unstable.
    #[inline(always)]
    pub fn sort_unstable_by<F>(&mut self, cmp: F)
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.0.sort_unstable_by(cmp);
    }

    /// Sort the key-value pairs of the map and return a by-value iterator of
    /// the key-value pairs with the result.
    ///
    /// The sort is unstable.
    #[inline(always)]
    pub fn sorted_unstable_by<F>(self, cmp: F) -> IntoIter<K, V>
    where
        F: FnMut(&K, &V, &K, &V) -> Ordering,
    {
        self.0.sorted_unstable_by(cmp)
    }

    /// Sort the map's key-value pairs in place using a sort-key extraction function.
    ///
    /// Computes in **O(n log n + c)** time where *n* is
    /// the length of the map and *c* is the capacity. The sort is unstable.
    #[inline(always)]
    pub fn sort_unstable_by_key<T, F>(&mut self, sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.0.sort_unstable_by_key(sort_key);
    }

    /// Sort the map's key-value pairs in place using a sort-key extraction function.
    ///
    /// During sorting, the function is called at most once per entry, by using temporary storage
    /// to remember the results of its evaluation. The order of calls to the function is
    /// unspecified and may change between versions of `indexmap` or the standard library.
    ///
    /// Computes in **O(m n + n log n + c)** time () and **O(n)** space, where the function is
    /// **O(m)**, *n* is the length of the map, and *c* the capacity. The sort is stable.
    #[inline(always)]
    pub fn sort_by_cached_key<T, F>(&mut self, sort_key: F)
    where
        T: Ord,
        F: FnMut(&K, &V) -> T,
    {
        self.0.sort_by_cached_key(sort_key);
    }

    /// Search over a sorted map for a key.
    ///
    /// Returns the position where that key is present, or the position where it can be inserted to
    /// maintain the sort. See [`slice::binary_search`] for more details.
    ///
    /// Computes in **O(log(n))** time, which is notably less scalable than looking the key up
    /// using [`get_index_of`][SparseIndexMap::get_index_of], but this can also position missing keys.
    #[inline(always)]
    pub fn binary_search_keys(&self, x: &K) -> Result<usize, usize>
    where
        K: Ord,
    {
        self.0.binary_search_keys(x)
    }

    /// Search over a sorted map with a comparator function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline(always)]
    pub fn binary_search_by<'a, F>(&'a self, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a K, &'a V) -> Ordering,
    {
        self.0.binary_search_by(f)
    }

    /// Search over a sorted map with an extraction function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by_key`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline(always)]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a K, &'a V) -> B,
        B: Ord,
    {
        self.0.binary_search_by_key(b, f)
    }

    /// Checks if the keys of this map are sorted.
    #[inline(always)]
    pub fn is_sorted(&self) -> bool
    where
        K: PartialOrd,
    {
        self.0.is_sorted()
    }

    /// Checks if this map is sorted using the given comparator function.
    #[inline(always)]
    pub fn is_sorted_by<'a, F>(&'a self, cmp: F) -> bool
    where
        F: FnMut(&'a K, &'a V, &'a K, &'a V) -> bool,
    {
        self.0.is_sorted_by(cmp)
    }

    /// Checks if this map is sorted using the given sort-key function.
    #[inline(always)]
    pub fn is_sorted_by_key<'a, F, T>(&'a self, sort_key: F) -> bool
    where
        F: FnMut(&'a K, &'a V) -> T,
        T: PartialOrd,
    {
        self.0.is_sorted_by_key(sort_key)
    }

    /// Returns the index of the partition point of a sorted map according to the given predicate
    /// (the index of the first element of the second partition).
    ///
    /// See `slice::partition_point` for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[must_use]
    #[inline(always)]
    pub fn partition_point<P>(&self, pred: P) -> usize
    where
        P: FnMut(&K, &V) -> bool,
    {
        self.0.partition_point(pred)
    }

    /// Reverses the order of the map's key-value pairs in place.
    ///
    /// Computes in **O(n)** time and **O(1)** space.
    #[inline(always)]
    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    /// Returns a slice of all the key-value pairs in the map.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn as_slice(&self) -> &Slice<K, V> {
        self.0.as_slice()
    }

    /// Returns a mutable slice of all the key-value pairs in the map.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut Slice<K, V> {
        self.0.as_mut_slice()
    }

    /// Converts into a boxed slice of all the key-value pairs in the map.
    ///
    /// Note that this will drop the inner hash table and any excess capacity.
    #[inline(always)]
    pub fn into_boxed_slice(self) -> Box<Slice<K, V>> {
        self.0.into_boxed_slice()
    }

    /// Get a key-value pair by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn get_index(&self, index: usize) -> Option<(&K, &V)> {
        self.0.get_index(index)
    }

    /// Get a key-value pair by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn get_index_mut(&mut self, index: usize) -> Option<(&K, &mut V)> {
        self.0.get_index_mut(index)
    }

    /// Get an entry in the map by index for in-place manipulation.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn get_index_entry(&mut self, index: usize) -> Option<IndexedEntry<'_, K, V>> {
        self.0.get_index_entry(index)
    }

    /// Get an array of `N` key-value pairs by `N` indices
    ///
    /// Valid indices are *0 <= index < self.len()* and each index needs to be unique.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::index::SparseIndexMap;
    /// let mut map = SparseIndexMap::from([(1, 'a'), (3, 'b'), (2, 'c')]);
    /// assert_eq!(map.get_disjoint_indices_mut([2, 0]), Ok([(&2, &mut 'c'), (&1, &mut 'a')]));
    /// ```
    #[inline(always)]
    pub fn get_disjoint_indices_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> Result<[(&K, &mut V); N], GetDisjointMutError> {
        self.0.get_disjoint_indices_mut(indices)
    }

    /// Returns a slice of key-value pairs in the given range of indices.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn get_range<R: RangeBounds<usize>>(&self, range: R) -> Option<&Slice<K, V>> {
        self.0.get_range(range)
    }

    /// Returns a mutable slice of key-value pairs in the given range of indices.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn get_range_mut<R: RangeBounds<usize>>(&mut self, range: R) -> Option<&mut Slice<K, V>> {
        self.0.get_range_mut(range)
    }

    /// Get the first key-value pair
    ///
    /// Computes in **O(1)** time.
    #[doc(alias = "first_key_value")] // like `BTreeMap`
    #[inline(always)]
    pub fn first(&self) -> Option<(&K, &V)> {
        self.0.first()
    }

    /// Get the first key-value pair, with mutable access to the value
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn first_mut(&mut self) -> Option<(&K, &mut V)> {
        self.0.first_mut()
    }

    /// Get the first entry in the map for in-place manipulation.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn first_entry(&mut self) -> Option<IndexedEntry<'_, K, V>> {
        self.0.first_entry()
    }

    /// Get the last key-value pair
    ///
    /// Computes in **O(1)** time.
    #[doc(alias = "last_key_value")] // like `BTreeMap`
    #[inline(always)]
    pub fn last(&self) -> Option<(&K, &V)> {
        self.0.last()
    }

    /// Get the last key-value pair, with mutable access to the value
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn last_mut(&mut self) -> Option<(&K, &mut V)> {
        self.0.last_mut()
    }

    /// Get the last entry in the map for in-place manipulation.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn last_entry(&mut self) -> Option<IndexedEntry<'_, K, V>> {
        self.0.last_entry()
    }

    /// Remove the key-value pair by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Like `Vec::swap_remove`, the pair is removed by swapping it with the
    /// last element of the map and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.0.swap_remove_index(index)
    }

    /// Remove the key-value pair by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Like `Vec::remove`, the pair is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn shift_remove_index(&mut self, index: usize) -> Option<(K, V)> {
        self.0.shift_remove_index(index)
    }

    /// Moves the position of a key-value pair from one index to another
    /// by shifting all other pairs in-between.
    ///
    /// * If `from < to`, the other pairs will shift down while the targeted pair moves up.
    /// * If `from > to`, the other pairs will shift up while the targeted pair moves down.
    ///
    /// ***Panics*** if `from` or `to` are out of bounds.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn move_index(&mut self, from: usize, to: usize) {
        self.0.move_index(from, to);
    }

    /// Swaps the position of two key-value pairs in the map.
    ///
    /// ***Panics*** if `a` or `b` are out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.0.swap_indices(a, b);
    }
}
