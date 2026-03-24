//! Provide [`SparseIndexSet`] based on [indexmap]'s implementation.
//!
//! Unlike [`indexmap::SparseIndexSet`], [`SparseIndexSet`] defaults to [`FixedHashState`]
//! instead of `RandomState`.
//!
//! This provides determinism by default with an acceptable compromise to denial
//! of service resistance in the context of a game engine.

use alloc::boxed::Box;
use core::cmp::Ordering;
use core::fmt::Debug;
use core::hash::Hash;
use core::ops::{BitAnd, BitOr, BitXor};
use core::ops::{Index, RangeBounds, Sub};

use indexmap::set::{Difference, Drain, ExtractIf, Intersection, Slice};
use indexmap::set::{IntoIter, Iter, Splice, SymmetricDifference, Union};
use indexmap::{Equivalent, TryReserveError};

use crate::hash::SparseHashState;

type InternalSet<T> = indexmap::IndexSet<T, SparseHashState>;

// -----------------------------------------------------------------------------
// SparseIndexSet

/// New-type for [`indexmap::IndexSet`] with [`SparseHashState`] as the default hashing provider.
///
/// Can be trivially converted to and from a [`indexmap::IndexSet`] using [`From`].
///
/// This provides determinism by default with an acceptable compromise to denial
/// of service resistance in the context of a game engine.
///
/// # Examples
///
/// ```
/// use voker_utils::index::SparseIndexSet;
///
/// let mut names = SparseIndexSet::new();
///
/// names.insert("a");
/// names.insert("b");
/// names.insert("c");
///
/// for name in &names {
///     // Fixed printing order,
///     // Must be a -> b -> c .
///     println!("{}", name);
/// }
/// ```
#[repr(transparent)]
pub struct SparseIndexSet<T>(InternalSet<T>);

// -----------------------------------------------------------------------------
// `FixedHashState` specific methods

impl<T: Eq + Hash, const N: usize> From<[T; N]> for SparseIndexSet<T> {
    fn from(value: [T; N]) -> Self {
        value.into_iter().collect()
    }
}

impl<T> SparseIndexSet<T> {
    /// Create a empty [`SparseIndexSet`]
    ///
    /// # Example
    ///
    /// ```rust
    /// use voker_utils::index::SparseIndexSet;
    ///
    /// let map = SparseIndexSet::new();
    /// #
    /// # let mut map = map;
    /// # map.insert("foo");
    /// # assert_eq!(map.get("foo"), Some("foo").as_ref());
    /// ```
    #[inline(always)]
    pub const fn new() -> Self {
        Self(InternalSet::with_hasher(SparseHashState))
    }

    /// Create a empty [`SparseIndexSet`] with specific capacity
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::index::SparseIndexSet;
    /// #
    /// let map = SparseIndexSet::with_capacity(5);
    /// #
    /// # let mut map = map;
    /// # map.insert("foo");
    /// # assert_eq!(map.get("foo"), Some("foo").as_ref());
    /// ```
    #[inline(always)]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(InternalSet::with_capacity_and_hasher(
            capacity,
            SparseHashState,
        ))
    }
}

// -----------------------------------------------------------------------------
// Transmute

// impl<T> Deref for SparseIndexSet<T> {
//     type Target = InternalSet<T>;
//
//     #[inline(always)]
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl<T> DerefMut for SparseIndexSet<T> {
//     #[inline(always)]
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0
//     }
// }

// -----------------------------------------------------------------------------
// Re-export the underlying method

impl<T> Clone for SparseIndexSet<T>
where
    InternalSet<T>: Clone,
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

impl<T> Debug for SparseIndexSet<T>
where
    InternalSet<T>: Debug,
{
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <InternalSet<T> as Debug>::fmt(&self.0, f)
    }
}

impl<T> Default for SparseIndexSet<T>
where
    InternalSet<T>: Default,
{
    #[inline(always)]
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T, I> Index<I> for SparseIndexSet<T>
where
    InternalSet<T>: Index<I>,
{
    type Output = <InternalSet<T> as Index<I>>::Output;

    #[inline(always)]
    fn index(&self, index: I) -> &Self::Output {
        self.0.index(index)
    }
}

impl<T> PartialEq for SparseIndexSet<T>
where
    InternalSet<T>: PartialEq,
{
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T> Eq for SparseIndexSet<T> where InternalSet<T>: Eq {}

impl<T, X> FromIterator<X> for SparseIndexSet<T>
where
    InternalSet<T>: FromIterator<X>,
{
    #[inline(always)]
    fn from_iter<U: IntoIterator<Item = X>>(iter: U) -> Self {
        Self(FromIterator::from_iter(iter))
    }
}

impl<T> IntoIterator for SparseIndexSet<T>
where
    InternalSet<T>: IntoIterator,
{
    type Item = <InternalSet<T> as IntoIterator>::Item;
    type IntoIter = <InternalSet<T> as IntoIterator>::IntoIter;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a SparseIndexSet<T>
where
    &'a InternalSet<T>: IntoIterator,
{
    type Item = <&'a InternalSet<T> as IntoIterator>::Item;
    type IntoIter = <&'a InternalSet<T> as IntoIterator>::IntoIter;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter()
    }
}

impl<'a, T> IntoIterator for &'a mut SparseIndexSet<T>
where
    &'a mut InternalSet<T>: IntoIterator,
{
    type Item = <&'a mut InternalSet<T> as IntoIterator>::Item;
    type IntoIter = <&'a mut InternalSet<T> as IntoIterator>::IntoIter;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        (&mut self.0).into_iter()
    }
}

impl<T, X> Extend<X> for SparseIndexSet<T>
where
    InternalSet<T>: Extend<X>,
{
    #[inline(always)]
    fn extend<U: IntoIterator<Item = X>>(&mut self, iter: U) {
        self.0.extend(iter);
    }
}

impl<T> serde_core::Serialize for SparseIndexSet<T>
where
    InternalSet<T>: serde_core::Serialize,
{
    #[inline(always)]
    fn serialize<U>(&self, serializer: U) -> Result<U::Ok, U::Error>
    where
        U: serde_core::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T> serde_core::Deserialize<'de> for SparseIndexSet<T>
where
    InternalSet<T>: serde_core::Deserialize<'de>,
{
    #[inline(always)]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde_core::Deserializer<'de>,
    {
        Ok(Self(serde_core::Deserialize::deserialize(deserializer)?))
    }
}

impl<T> SparseIndexSet<T> {
    /// Return the number of elements the set can hold without reallocating.
    ///
    /// This number is a lower bound; the set might be able to hold more,
    /// but is guaranteed to be able to hold at least this many.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// Return the number of elements in the set.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the set contains no elements.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return an iterator over the values of the set, in their order
    #[inline(always)]
    pub fn iter(&self) -> Iter<'_, T> {
        self.0.iter()
    }

    /// Remove all elements in the set, while preserving its capacity.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Shortens the set, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater than the set's current length, this has no effect.
    #[inline(always)]
    pub fn truncate(&mut self, len: usize) {
        self.0.truncate(len);
    }

    /// Clears the `SparseIndexSet` in the given index range, returning those values
    /// as a drain iterator.
    ///
    /// The range may be any type that implements [`RangeBounds<usize>`],
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To drain the set entirely, use `RangeFull`
    /// like `set.drain(..)`.
    ///
    /// This shifts down all entries following the drained range to fill the
    /// gap, and keeps the allocated memory for reuse.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the set.
    #[inline(always)]
    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T>
    where
        R: RangeBounds<usize>,
    {
        self.0.drain(range)
    }

    /// Creates an iterator which uses a closure to determine if a value should be removed,
    /// for all values in the given range.
    ///
    /// If the closure returns true, then the value is removed and yielded.
    /// If the closure returns false, the value will remain in the list and will not be yielded
    /// by the iterator.
    ///
    /// The range may be any type that implements [`RangeBounds<usize>`],
    /// including all of the `std::ops::Range*` types, or even a tuple pair of
    /// `Bound` start and end values. To check the entire set, use `RangeFull`
    /// like `set.extract_if(.., predicate)`.
    ///
    /// If the returned `ExtractIf` is not exhausted, e.g. because it is dropped without iterating
    /// or the iteration short-circuits, then the remaining elements will be retained.
    /// Use [`retain`] with a negated predicate if you do not need the returned iterator.
    ///
    /// [`retain`]: SparseIndexSet::retain
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the set.
    ///
    /// # Examples
    ///
    /// Splitting a set into even and odd values, reusing the original set:
    ///
    /// ```
    /// use voker_utils::index::SparseIndexSet;
    ///
    /// let mut set: SparseIndexSet<i32> = (0..8).collect();
    /// let extracted: SparseIndexSet<i32> = set.extract_if(.., |v| v % 2 == 0).collect();
    ///
    /// let evens = extracted.into_iter().collect::<Vec<_>>();
    /// let odds = set.into_iter().collect::<Vec<_>>();
    ///
    /// assert_eq!(evens, vec![0, 2, 4, 6]);
    /// assert_eq!(odds, vec![1, 3, 5, 7]);
    /// ```
    #[inline(always)]
    pub fn extract_if<F, R>(&mut self, range: R, pred: F) -> ExtractIf<'_, T, F>
    where
        F: FnMut(&T) -> bool,
        R: RangeBounds<usize>,
    {
        self.0.extract_if(range, pred)
    }

    /// Splits the collection into two at the given index.
    ///
    /// Returns a newly allocated set containing the elements in the range
    /// `[at, len)`. After the call, the original set will be left containing
    /// the elements `[0, at)` with its previous capacity unchanged.
    ///
    /// ***Panics*** if `at > len`.
    #[inline(always)]
    pub fn split_off(&mut self, at: usize) -> Self {
        Self(self.0.split_off(at))
    }

    /// Reserve capacity for `additional` more values.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Reserve capacity for `additional` more values, without over-allocating.
    ///
    /// Unlike `reserve`, this does not deliberately over-allocate the entry capacity to avoid
    /// frequent re-allocations. However, the underlying data structures may still have internal
    /// capacity requirements, and the allocator itself may give more space than requested, so this
    /// cannot be relied upon to be precisely minimal.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.0.reserve_exact(additional);
    }

    /// Try to reserve capacity for `additional` more values.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve(additional)
    }

    /// Try to reserve capacity for `additional` more values, without over-allocating.
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

    /// Shrink the capacity of the set as much as possible.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }

    /// Shrink the capacity of the set with a lower limit.
    ///
    /// Computes in **O(n)** time.
    #[inline(always)]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.0.shrink_to(min_capacity);
    }
}

impl<T> SparseIndexSet<T>
where
    T: Hash + Eq,
{
    /// Insert the value into the set.
    ///
    /// If an equivalent item already exists in the set, it returns
    /// `false` leaving the original value in the set and without
    /// altering its insertion order. Otherwise, it inserts the new
    /// item and returns `true`.
    ///
    /// Computes in **O(1)** time (amortized average).
    #[inline(always)]
    pub fn insert(&mut self, value: T) -> bool {
        self.0.insert(value)
    }

    /// Insert the value into the set, and get its index.
    ///
    /// If an equivalent item already exists in the set, it returns
    /// the index of the existing item and `false`, leaving the
    /// original value in the set and without altering its insertion
    /// order. Otherwise, it inserts the new item and returns the index
    /// of the inserted item and `true`.
    ///
    /// Computes in **O(1)** time (amortized average).
    #[inline(always)]
    pub fn insert_full(&mut self, value: T) -> (usize, bool) {
        self.0.insert_full(value)
    }

    /// Insert the value into the set at its ordered position among sorted values.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search`][Self::binary_search], and if needed calling
    /// [`insert_before`][Self::insert_before] for a new value.
    ///
    /// If the sorted item is found in the set, it returns the index of that
    /// existing item and `false`, without any change. Otherwise, it inserts the
    /// new item and returns its sorted index and `true`.
    ///
    /// If the existing items are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the value
    /// is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average). Instead of repeating calls to
    /// `insert_sorted`, it may be faster to call batched [`insert`][Self::insert]
    /// or [`extend`][Self::extend] and only call [`sort`][Self::sort] or
    /// [`sort_unstable`][Self::sort_unstable] once.
    #[inline(always)]
    pub fn insert_sorted(&mut self, value: T) -> (usize, bool)
    where
        T: Ord,
    {
        self.0.insert_sorted(value)
    }

    /// Insert the value into the set at its ordered position among values
    /// sorted by `cmp`.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_by`][Self::binary_search_by], then calling
    /// [`insert_before`][Self::insert_before].
    ///
    /// If the existing items are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the value
    /// is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn insert_sorted_by<F>(&mut self, value: T, cmp: F) -> (usize, bool)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.0.insert_sorted_by(value, cmp)
    }

    /// Insert the value into the set at its ordered position among values
    /// using a sort-key extraction function.
    ///
    /// This is equivalent to finding the position with
    /// [`binary_search_by_key`][Self::binary_search_by_key] with `sort_key(key)`,
    /// then calling [`insert_before`][Self::insert_before].
    ///
    /// If the existing items are **not** already sorted, then the insertion
    /// index is unspecified (like [`slice::binary_search`]), but the value
    /// is moved to or inserted at that position regardless.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn insert_sorted_by_key<B, F>(&mut self, value: T, sort_key: F) -> (usize, bool)
    where
        B: Ord,
        F: FnMut(&T) -> B,
    {
        self.0.insert_sorted_by_key(value, sort_key)
    }

    /// Insert the value into the set before the value at the given index, or at the end.
    ///
    /// If an equivalent item already exists in the set, it returns `false` leaving the
    /// original value in the set, but moved to the new position. The returned index
    /// will either be the given index or one less, depending on how the value moved.
    /// (See [`shift_insert`](Self::shift_insert) for different behavior here.)
    ///
    /// Otherwise, it inserts the new value exactly at the given index and returns `true`.
    ///
    /// ***Panics*** if `index` is out of bounds.
    /// Valid indices are `0..=set.len()` (inclusive).
    ///
    /// Computes in **O(n)** time (average).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexSet;
    /// let mut set: SparseIndexSet<char> = ('a'..='z').collect();
    ///
    /// // The new value '*' goes exactly at the given index.
    /// assert_eq!(set.get_index_of(&'*'), None);
    /// assert_eq!(set.insert_before(10, '*'), (10, true));
    /// assert_eq!(set.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the value 'a' up will shift others down, so this moves *before* 10 to index 9.
    /// assert_eq!(set.insert_before(10, 'a'), (9, false));
    /// assert_eq!(set.get_index_of(&'a'), Some(9));
    /// assert_eq!(set.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the value 'z' down will shift others up, so this moves to exactly 10.
    /// assert_eq!(set.insert_before(10, 'z'), (10, false));
    /// assert_eq!(set.get_index_of(&'z'), Some(10));
    /// assert_eq!(set.get_index_of(&'*'), Some(11));
    ///
    /// // Moving or inserting before the endpoint is also valid.
    /// assert_eq!(set.len(), 27);
    /// assert_eq!(set.insert_before(set.len(), '*'), (26, false));
    /// assert_eq!(set.get_index_of(&'*'), Some(26));
    /// assert_eq!(set.insert_before(set.len(), '+'), (27, true));
    /// assert_eq!(set.get_index_of(&'+'), Some(27));
    /// assert_eq!(set.len(), 28);
    /// ```
    #[inline(always)]
    pub fn insert_before(&mut self, index: usize, value: T) -> (usize, bool) {
        self.0.insert_before(index, value)
    }

    /// Insert the value into the set at the given index.
    ///
    /// If an equivalent item already exists in the set, it returns `false` leaving
    /// the original value in the set, but moved to the given index.
    /// Note that existing values **cannot** be moved to `index == set.len()`!
    /// (See [`insert_before`](Self::insert_before) for different behavior here.)
    ///
    /// Otherwise, it inserts the new value at the given index and returns `true`.
    ///
    /// ***Panics*** if `index` is out of bounds.
    /// Valid indices are `0..set.len()` (exclusive) when moving an existing value, or
    /// `0..=set.len()` (inclusive) when inserting a new value.
    ///
    /// Computes in **O(n)** time (average).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexSet;
    /// let mut set: SparseIndexSet<char> = ('a'..='z').collect();
    ///
    /// // The new value '*' goes exactly at the given index.
    /// assert_eq!(set.get_index_of(&'*'), None);
    /// assert_eq!(set.shift_insert(10, '*'), true);
    /// assert_eq!(set.get_index_of(&'*'), Some(10));
    ///
    /// // Moving the value 'a' up to 10 will shift others down, including the '*' that was at 10.
    /// assert_eq!(set.shift_insert(10, 'a'), false);
    /// assert_eq!(set.get_index_of(&'a'), Some(10));
    /// assert_eq!(set.get_index_of(&'*'), Some(9));
    ///
    /// // Moving the value 'z' down to 9 will shift others up, including the '*' that was at 9.
    /// assert_eq!(set.shift_insert(9, 'z'), false);
    /// assert_eq!(set.get_index_of(&'z'), Some(9));
    /// assert_eq!(set.get_index_of(&'*'), Some(10));
    ///
    /// // Existing values can move to len-1 at most, but new values can insert at the endpoint.
    /// assert_eq!(set.len(), 27);
    /// assert_eq!(set.shift_insert(set.len() - 1, '*'), false);
    /// assert_eq!(set.get_index_of(&'*'), Some(26));
    /// assert_eq!(set.shift_insert(set.len(), '+'), true);
    /// assert_eq!(set.get_index_of(&'+'), Some(27));
    /// assert_eq!(set.len(), 28);
    /// ```
    #[inline(always)]
    pub fn shift_insert(&mut self, index: usize, value: T) -> bool {
        self.0.shift_insert(index, value)
    }

    /// Adds a value to the set, replacing the existing value, if any, that is
    /// equal to the given one, without altering its insertion order. Returns
    /// the replaced value.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn replace(&mut self, value: T) -> Option<T> {
        self.0.replace(value)
    }

    /// Adds a value to the set, replacing the existing value, if any, that is
    /// equal to the given one, without altering its insertion order. Returns
    /// the index of the item and its replaced value.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn replace_full(&mut self, value: T) -> (usize, Option<T>) {
        self.0.replace_full(value)
    }

    /// Replaces the value at the given index. The new value does not need to be
    /// equivalent to the one it is replacing, but it must be unique to the rest
    /// of the set.
    ///
    /// Returns `Ok(old_value)` if successful, or `Err((other_index, value))` if
    /// an equivalent value already exists at a different index. The set will be
    /// unchanged in the error case.
    ///
    /// ***Panics*** if `index` is out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn replace_index(&mut self, index: usize, value: T) -> Result<T, (usize, T)> {
        self.0.replace_index(index, value)
    }

    /// Return an iterator over the values that are in `self` but not `other`.
    ///
    /// Values are produced in the same order that they appear in `self`.
    #[inline(always)]
    pub fn difference<'a>(
        &'a self,
        other: &'a SparseIndexSet<T>,
    ) -> Difference<'a, T, SparseHashState> {
        self.0.difference(&other.0)
    }

    /// Return an iterator over the values that are in `self` or `other`,
    /// but not in both.
    ///
    /// Values from `self` are produced in their original order, followed by
    /// values from `other` in their original order.
    #[inline(always)]
    pub fn symmetric_difference<'a>(
        &'a self,
        other: &'a SparseIndexSet<T>,
    ) -> SymmetricDifference<'a, T, SparseHashState, SparseHashState> {
        self.0.symmetric_difference(&other.0)
    }

    /// Return an iterator over the values that are in both `self` and `other`.
    ///
    /// Values are produced in the same order that they appear in `self`.
    #[inline(always)]
    pub fn intersection<'a>(
        &'a self,
        other: &'a SparseIndexSet<T>,
    ) -> Intersection<'a, T, SparseHashState> {
        self.0.intersection(&other.0)
    }

    /// Return an iterator over all values that are in `self` or `other`.
    ///
    /// Values from `self` are produced in their original order, followed by
    /// values that are unique to `other` in their original order.
    #[inline(always)]
    pub fn union<'a>(&'a self, other: &'a SparseIndexSet<T>) -> Union<'a, T, SparseHashState> {
        self.0.union(&other.0)
    }

    /// Creates a splicing iterator that replaces the specified range in the set
    /// with the given `replace_with` iterator and yields the removed items.
    /// `replace_with` does not need to be the same length as `range`.
    ///
    /// The `range` is removed even if the iterator is not consumed until the
    /// end. It is unspecified how many elements are removed from the set if the
    /// `Splice` value is leaked.
    ///
    /// The input iterator `replace_with` is only consumed when the `Splice`
    /// value is dropped. If a value from the iterator matches an existing entry
    /// in the set (outside of `range`), then the original will be unchanged.
    /// Otherwise, the new value will be inserted in the replaced `range`.
    ///
    /// ***Panics*** if the starting point is greater than the end point or if
    /// the end point is greater than the length of the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexSet;
    ///
    /// let mut set = SparseIndexSet::from([0, 1, 2, 3, 4]);
    /// let new = [5, 4, 3, 2, 1];
    /// let removed: Vec<_> = set.splice(2..4, new).collect();
    ///
    /// // 1 and 4 kept their positions, while 5, 3, and 2 were newly inserted.
    /// assert!(set.into_iter().eq([0, 1, 5, 3, 2, 4]));
    /// assert_eq!(removed, &[2, 3]);
    /// ```
    #[inline(always)]
    pub fn splice<R, I>(
        &mut self,
        range: R,
        replace_with: I,
    ) -> Splice<'_, I::IntoIter, T, SparseHashState>
    where
        R: RangeBounds<usize>,
        I: IntoIterator<Item = T>,
    {
        self.0.splice(range, replace_with)
    }

    /// Moves all values from `other` into `self`, leaving `other` empty.
    ///
    /// This is equivalent to calling [`insert`][Self::insert] for each value
    /// from `other` in order, which means that values that already exist
    /// in `self` are unchanged in their current position.
    ///
    /// See also [`union`][Self::union] to iterate the combined values by
    /// reference, without modifying `self` or `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexSet;
    ///
    /// let mut a = SparseIndexSet::from([3, 2, 1]);
    /// let mut b = SparseIndexSet::from([3, 4, 5]);
    /// let old_capacity = b.capacity();
    ///
    /// a.append(&mut b);
    ///
    /// assert_eq!(a.len(), 5);
    /// assert_eq!(b.len(), 0);
    /// assert_eq!(b.capacity(), old_capacity);
    ///
    /// assert!(a.iter().eq(&[3, 2, 1, 4, 5]));
    /// ```
    #[inline(always)]
    pub fn append(&mut self, other: &mut SparseIndexSet<T>) {
        self.0.append(&mut other.0);
    }
}

impl<T> SparseIndexSet<T> {
    /// Return `true` if an equivalent to `value` exists in the set.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.contains(value)
    }

    /// Return a reference to the value stored in the set, if it is present,
    /// else `None`.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.get(value)
    }

    /// Return item index and value
    #[inline(always)]
    pub fn get_full<Q>(&self, value: &Q) -> Option<(usize, &T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.get_full(value)
    }

    /// Return item index, if it exists in the set
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn get_index_of<Q>(&self, value: &Q) -> Option<usize>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.get_index_of(value)
    }

    /// Remove the value from the set, and return `true` if it was present.
    ///
    /// Like `Vec::swap_remove`, the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `false` if `value` was not in the set.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_remove<Q>(&mut self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.swap_remove(value)
    }

    /// Remove the value from the set, and return `true` if it was present.
    ///
    /// Like `Vec::remove`, the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `false` if `value` was not in the set.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn shift_remove<Q>(&mut self, value: &Q) -> bool
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.shift_remove(value)
    }

    /// Removes and returns the value in the set, if any, that is equal to the
    /// given one.
    ///
    /// Like `Vec::swap_remove`, the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `value` was not in the set.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_take<Q>(&mut self, value: &Q) -> Option<T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.swap_take(value)
    }

    /// Removes and returns the value in the set, if any, that is equal to the
    /// given one.
    ///
    /// Like `Vec::remove`, the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `value` was not in the set.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn shift_take<Q>(&mut self, value: &Q) -> Option<T>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.shift_take(value)
    }

    /// Remove the value from the set return it and the index it had.
    ///
    /// Like `Vec::swap_remove`, the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Return `None` if `value` was not in the set.
    #[inline(always)]
    pub fn swap_remove_full<Q>(&mut self, value: &Q) -> Option<(usize, T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.swap_remove_full(value)
    }

    /// Remove the value from the set return it and the index it had.
    ///
    /// Like `Vec::remove`, the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Return `None` if `value` was not in the set.
    #[inline(always)]
    pub fn shift_remove_full<Q>(&mut self, value: &Q) -> Option<(usize, T)>
    where
        Q: ?Sized + Hash + Equivalent<T>,
    {
        self.0.shift_remove_full(value)
    }
}

impl<T> SparseIndexSet<T> {
    /// Remove the last value
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    #[doc(alias = "pop_last")] // like `BTreeSet`
    #[inline(always)]
    pub fn pop(&mut self) -> Option<T> {
        self.0.pop()
    }

    /// Removes and returns the last value from a set if the predicate
    /// returns `true`, or [`None`] if the predicate returns false or the set
    /// is empty (the predicate will not be called in that case).
    ///
    /// This preserves the order of the remaining elements.
    ///
    /// Computes in **O(1)** time (average).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::index::SparseIndexSet;
    ///
    /// let mut set = SparseIndexSet::from([1, 2, 3, 4]);
    /// let pred = |x: &i32| *x % 2 == 0;
    ///
    /// assert_eq!(set.pop_if(pred), Some(4));
    /// assert_eq!(set.as_slice(), &[1, 2, 3]);
    /// assert_eq!(set.pop_if(pred), None);
    /// ```
    #[inline(always)]
    pub fn pop_if(&mut self, predicate: impl FnOnce(&T) -> bool) -> Option<T> {
        self.0.pop_if(predicate)
    }

    /// Scan through each value in the set and keep those where the
    /// closure `keep` returns `true`.
    ///
    /// The elements are visited in order, and remaining elements keep their
    /// order.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn retain<F>(&mut self, keep: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.0.retain(keep);
    }

    /// Sort the set's values by their default ordering.
    ///
    /// This is a stable sort -- but equivalent values should not normally coexist in
    /// a set at all, so [`sort_unstable`][Self::sort_unstable] is preferred
    /// because it is generally faster and doesn't allocate auxiliary memory.
    ///
    /// See [`sort_by`](Self::sort_by) for details.
    #[inline(always)]
    pub fn sort(&mut self)
    where
        T: Ord,
    {
        self.0.sort();
    }

    /// Sort the set's values in place using the comparison function `cmp`.
    ///
    /// Computes in **O(n log n)** time and **O(n)** space. The sort is stable.
    #[inline(always)]
    pub fn sort_by<F>(&mut self, cmp: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.0.sort_by(cmp);
    }

    /// Sort the values of the set and return a by-value iterator of
    /// the values with the result.
    ///
    /// The sort is stable.
    #[inline(always)]
    pub fn sorted_by<F>(self, cmp: F) -> IntoIter<T>
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.0.sorted_by(cmp)
    }

    /// Sort the set's values in place using a key extraction function.
    ///
    /// Computes in **O(n log n)** time and **O(n)** space. The sort is stable.
    #[inline(always)]
    pub fn sort_by_key<K, F>(&mut self, sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.0.sort_by_key(sort_key);
    }

    /// Sort the set's values by their default ordering.
    ///
    /// See [`sort_unstable_by`](Self::sort_unstable_by) for details.
    #[inline(always)]
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        self.0.sort_unstable();
    }

    /// Sort the set's values in place using the comparison function `cmp`.
    ///
    /// Computes in **O(n log n)** time. The sort is unstable.
    #[inline(always)]
    pub fn sort_unstable_by<F>(&mut self, cmp: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.0.sort_unstable_by(cmp);
    }

    /// Sort the values of the set and return a by-value iterator of
    /// the values with the result.
    #[inline(always)]
    pub fn sorted_unstable_by<F>(self, cmp: F) -> IntoIter<T>
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        self.0.sorted_unstable_by(cmp)
    }

    /// Sort the set's values in place using a key extraction function.
    ///
    /// Computes in **O(n log n)** time. The sort is unstable.
    #[inline(always)]
    pub fn sort_unstable_by_key<K, F>(&mut self, sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.0.sort_unstable_by_key(sort_key);
    }

    /// Sort the set's values in place using a key extraction function.
    ///
    /// During sorting, the function is called at most once per entry, by using temporary storage
    /// to remember the results of its evaluation. The order of calls to the function is
    /// unspecified and may change between versions of `indexmap` or the standard library.
    ///
    /// Computes in **O(m n + n log n + c)** time () and **O(n)** space, where the function is
    /// **O(m)**, *n* is the length of the map, and *c* the capacity. The sort is stable.
    #[inline(always)]
    pub fn sort_by_cached_key<K, F>(&mut self, sort_key: F)
    where
        K: Ord,
        F: FnMut(&T) -> K,
    {
        self.0.sort_by_cached_key(sort_key);
    }

    /// Search over a sorted set for a value.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search`] for more details.
    ///
    /// Computes in **O(log(n))** time, which is notably less scalable than looking the value up
    /// using [`get_index_of`][SparseIndexSet::get_index_of], but this can also position missing values.
    #[inline(always)]
    pub fn binary_search(&self, x: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.0.binary_search(x)
    }

    /// Search over a sorted set with a comparator function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline(always)]
    pub fn binary_search_by<'a, F>(&'a self, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> Ordering,
    {
        self.0.binary_search_by(f)
    }

    /// Search over a sorted set with an extraction function.
    ///
    /// Returns the position where that value is present, or the position where it can be inserted
    /// to maintain the sort. See [`slice::binary_search_by_key`] for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline(always)]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> B,
        B: Ord,
    {
        self.0.binary_search_by_key(b, f)
    }

    /// Checks if the values of this set are sorted.
    #[inline(always)]
    pub fn is_sorted(&self) -> bool
    where
        T: PartialOrd,
    {
        self.0.is_sorted()
    }

    /// Checks if this set is sorted using the given comparator function.
    #[inline(always)]
    pub fn is_sorted_by<'a, F>(&'a self, cmp: F) -> bool
    where
        F: FnMut(&'a T, &'a T) -> bool,
    {
        self.0.is_sorted_by(cmp)
    }

    /// Checks if this set is sorted using the given sort-key function.
    #[inline(always)]
    pub fn is_sorted_by_key<'a, F, K>(&'a self, sort_key: F) -> bool
    where
        F: FnMut(&'a T) -> K,
        K: PartialOrd,
    {
        self.0.is_sorted_by_key(sort_key)
    }

    /// Returns the index of the partition point of a sorted set according to the given predicate
    /// (the index of the first element of the second partition).
    ///
    /// See `slice::partition_point` for more details.
    ///
    /// Computes in **O(log(n))** time.
    #[inline(always)]
    pub fn partition_point<P>(&self, pred: P) -> usize
    where
        P: FnMut(&T) -> bool,
    {
        self.0.partition_point(pred)
    }

    /// Reverses the order of the set's values in place.
    ///
    /// Computes in **O(n)** time and **O(1)** space.
    #[inline(always)]
    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    /// Returns a slice of all the values in the set.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn as_slice(&self) -> &Slice<T> {
        self.0.as_slice()
    }

    /// Converts into a boxed slice of all the values in the set.
    ///
    /// Note that this will drop the inner hash table and any excess capacity.
    #[inline(always)]
    pub fn into_boxed_slice(self) -> Box<Slice<T>> {
        self.0.into_boxed_slice()
    }

    /// Get a value by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn get_index(&self, index: usize) -> Option<&T> {
        self.0.get_index(index)
    }

    /// Returns a slice of values in the given range of indices.
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn get_range<R: RangeBounds<usize>>(&self, range: R) -> Option<&Slice<T>> {
        self.0.get_range(range)
    }

    /// Get the first value
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn first(&self) -> Option<&T> {
        self.0.first()
    }

    /// Get the last value
    ///
    /// Computes in **O(1)** time.
    #[inline(always)]
    pub fn last(&self) -> Option<&T> {
        self.0.last()
    }

    /// Remove the value by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Like `Vec::swap_remove`, the value is removed by swapping it with the
    /// last element of the set and popping it off. **This perturbs
    /// the position of what used to be the last element!**
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_remove_index(&mut self, index: usize) -> Option<T> {
        self.0.swap_remove_index(index)
    }

    /// Remove the value by index
    ///
    /// Valid indices are `0 <= index < self.len()`.
    ///
    /// Like `Vec::remove`, the value is removed by shifting all of the
    /// elements that follow it, preserving their relative order.
    /// **This perturbs the index of all of those elements!**
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn shift_remove_index(&mut self, index: usize) -> Option<T> {
        self.0.shift_remove_index(index)
    }

    /// Moves the position of a value from one index to another
    /// by shifting all other values in-between.
    ///
    /// * If `from < to`, the other values will shift down while the targeted value moves up.
    /// * If `from > to`, the other values will shift up while the targeted value moves down.
    ///
    /// ***Panics*** if `from` or `to` are out of bounds.
    ///
    /// Computes in **O(n)** time (average).
    #[inline(always)]
    pub fn move_index(&mut self, from: usize, to: usize) {
        self.0.move_index(from, to)
    }

    /// Swaps the position of two values in the set.
    ///
    /// ***Panics*** if `a` or `b` are out of bounds.
    ///
    /// Computes in **O(1)** time (average).
    #[inline(always)]
    pub fn swap_indices(&mut self, a: usize, b: usize) {
        self.0.swap_indices(a, b);
    }
}

impl<T> SparseIndexSet<T>
where
    T: Eq + Hash,
{
    /// Returns `true` if `self` has no elements in common with `other`.
    #[inline(always)]
    pub fn is_disjoint(&self, other: &SparseIndexSet<T>) -> bool {
        self.0.is_disjoint(&other.0)
    }

    /// Returns `true` if all elements of `self` are contained in `other`.
    #[inline(always)]
    pub fn is_subset(&self, other: &SparseIndexSet<T>) -> bool {
        self.0.is_subset(&other.0)
    }

    /// Returns `true` if all elements of `other` are contained in `self`.
    #[inline(always)]
    pub fn is_superset(&self, other: &SparseIndexSet<T>) -> bool {
        self.0.is_superset(&other.0)
    }
}

impl<T> BitAnd<&SparseIndexSet<T>> for &SparseIndexSet<T>
where
    T: Eq + Hash + Clone,
{
    type Output = SparseIndexSet<T>;

    /// Returns the set intersection, cloned into a new set.
    ///
    /// Values are collected in the same order that they appear in `self`.
    fn bitand(self, other: &SparseIndexSet<T>) -> Self::Output {
        self.intersection(other).cloned().collect()
    }
}

impl<T> BitOr<&SparseIndexSet<T>> for &SparseIndexSet<T>
where
    T: Eq + Hash + Clone,
{
    type Output = SparseIndexSet<T>;

    /// Returns the set union, cloned into a new set.
    ///
    /// Values from `self` are collected in their original order, followed by
    /// values that are unique to `other` in their original order.
    fn bitor(self, other: &SparseIndexSet<T>) -> Self::Output {
        self.union(other).cloned().collect()
    }
}

impl<T> BitXor<&SparseIndexSet<T>> for &SparseIndexSet<T>
where
    T: Eq + Hash + Clone,
{
    type Output = SparseIndexSet<T>;

    /// Returns the set symmetric-difference, cloned into a new set.
    ///
    /// Values from `self` are collected in their original order, followed by
    /// values from `other` in their original order.
    fn bitxor(self, other: &SparseIndexSet<T>) -> Self::Output {
        self.symmetric_difference(other).cloned().collect()
    }
}

impl<T> Sub<&SparseIndexSet<T>> for &SparseIndexSet<T>
where
    T: Eq + Hash + Clone,
{
    type Output = SparseIndexSet<T>;

    /// Returns the set difference, cloned into a new set.
    ///
    /// Values are collected in the same order that they appear in `self`.
    fn sub(self, other: &SparseIndexSet<T>) -> Self::Output {
        self.difference(other).cloned().collect()
    }
}
