//! Provide [`SparseHashSet`] based on [hashbrown]'s implementation.

use core::fmt::Debug;
use core::hash::Hash;
use core::ops::{BitAnd, BitAndAssign};
use core::ops::{BitOr, BitOrAssign};
use core::ops::{BitXor, BitXorAssign};
use core::ops::{Sub, SubAssign};

use hashbrown::{Equivalent, TryReserveError, hash_set as hb};
use hb::{Difference, Intersection};
use hb::{Drain, ExtractIf, Iter};
use hb::{SymmetricDifference, Union};

use crate::hash::SparseHashState;

// -----------------------------------------------------------------------------
// SparseHashSet

type InternalSet<T> = hb::HashSet<T, SparseHashState>;

/// New-type for [`HashSet`] with [`SparseHashState`] as the default hashing provider.
///
/// # Examples
///
/// ```
/// use voker_utils::hash::SparseHashSet;
///
/// let mut names = SparseHashSet::new();
///
/// names.insert("a");
/// names.insert("b");
/// names.insert("c");
///
/// for name in &names {
///     // Fixed printing order,
///     // but may not a -> b -> c .
///     println!("{}", name);
/// }
/// ```
///
/// [`HashSet`]: hb::HashSet
#[repr(transparent)]
pub struct SparseHashSet<T>(InternalSet<T>);

// -----------------------------------------------------------------------------
// `SparseHashState` specific methods

impl<T: Eq + Hash, const N: usize> From<[T; N]> for SparseHashSet<T> {
    fn from(value: [T; N]) -> Self {
        value.into_iter().collect()
    }
}

impl<T> SparseHashSet<T> {
    /// Create a empty [`SparseHashSet`]
    ///
    /// # Example
    ///
    /// ```rust
    /// use voker_utils::hash::SparseHashSet;
    ///
    /// let map = SparseHashSet::new();
    /// #
    /// # let mut map = map;
    /// # map.insert("foo");
    /// # assert_eq!(map.get("foo"), Some("foo").as_ref());
    /// ```
    #[inline(always)]
    pub const fn new() -> Self {
        Self(InternalSet::with_hasher(SparseHashState))
    }

    /// Create a empty [`SparseHashSet`] with specific capacity
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// #
    /// let map = SparseHashSet::with_capacity(5);
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

// impl<T> Deref for SparseHashSet<T> {
//     type Target = InternalSet<T>;
//
//     #[inline(always)]
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl<T> DerefMut for SparseHashSet<T> {
//     #[inline(always)]
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.0
//     }
// }

// -----------------------------------------------------------------------------
// Re-export the underlying method

impl<T> Clone for SparseHashSet<T>
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

impl<T> Debug for SparseHashSet<T>
where
    InternalSet<T>: Debug,
{
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <InternalSet<T> as Debug>::fmt(&self.0, f)
    }
}

impl<T> Default for SparseHashSet<T>
where
    InternalSet<T>: Default,
{
    #[inline(always)]
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T> PartialEq for SparseHashSet<T>
where
    InternalSet<T>: PartialEq,
{
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T> Eq for SparseHashSet<T> where InternalSet<T>: Eq {}

impl<T, X> FromIterator<X> for SparseHashSet<T>
where
    InternalSet<T>: FromIterator<X>,
{
    #[inline(always)]
    fn from_iter<U: IntoIterator<Item = X>>(iter: U) -> Self {
        Self(FromIterator::from_iter(iter))
    }
}

impl<T> IntoIterator for SparseHashSet<T>
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

impl<'a, T> IntoIterator for &'a SparseHashSet<T>
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

impl<'a, T> IntoIterator for &'a mut SparseHashSet<T>
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

impl<T, X> Extend<X> for SparseHashSet<T>
where
    InternalSet<T>: Extend<X>,
{
    #[inline(always)]
    fn extend<U: IntoIterator<Item = X>>(&mut self, iter: U) {
        self.0.extend(iter);
    }
}

impl<T> serde_core::Serialize for SparseHashSet<T>
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

impl<'de, T> serde_core::Deserialize<'de> for SparseHashSet<T>
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

impl<T> SparseHashSet<T> {
    /// Returns the number of elements the set can hold without reallocating.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let map = SparseHashSet::with_capacity(5);
    ///
    /// # let map: SparseHashSet<()> = map;
    /// #
    /// assert!(map.capacity() >= 5);
    /// ```
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.0.capacity()
    }

    /// An iterator visiting all elements in arbitrary order.
    /// The iterator element type is `&'a T`
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    /// map.insert("bar");
    /// map.insert("baz");
    ///
    /// for value in map.iter() {
    ///     // "foo", "bar", "baz" (arbitrary order)
    /// }
    /// # assert_eq!(map.iter().count(), 3);
    /// ```
    #[inline(always)]
    pub fn iter(&self) -> Iter<'_, T> {
        self.0.iter()
    }

    /// Returns the number of elements in the set.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// assert_eq!(map.len(), 0);
    ///
    /// map.insert("foo");
    ///
    /// assert_eq!(map.len(), 1);
    /// ```
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the set contains no elements.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// assert!(map.is_empty());
    ///
    /// map.insert("foo");
    ///
    /// assert!(!map.is_empty());
    /// ```
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Clears the set, returning all elements in an iterator.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// #
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    /// map.insert("bar");
    /// map.insert("baz");
    ///
    /// for value in map.drain() {
    ///     // "foo", "bar", "baz"
    ///     // arbitrary order
    /// }
    ///
    /// assert!(map.is_empty());
    /// ```
    #[inline(always)]
    pub fn drain(&mut self) -> Drain<'_, T> {
        self.0.drain()
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// #
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    /// map.insert("bar");
    /// map.insert("baz");
    ///
    /// map.retain(|value| *value == "baz");
    ///
    /// assert_eq!(map.len(), 1);
    /// ```
    #[inline(always)]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.0.retain(f);
    }

    /// Drains elements which are true under the given predicate,
    /// and returns an iterator over the removed items.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// #
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    /// map.insert("bar");
    /// map.insert("baz");
    ///
    /// let extracted = map
    ///     .extract_if(|value| *value == "baz")
    ///     .collect::<Vec<_>>();
    ///
    /// assert_eq!(map.len(), 2);
    /// assert_eq!(extracted.len(), 1);
    /// ```
    #[inline(always)]
    pub fn extract_if<F>(&mut self, f: F) -> ExtractIf<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        self.0.extract_if(f)
    }

    /// Clears the set, removing all values.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// #
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    /// map.insert("bar");
    /// map.insert("baz");
    ///
    /// map.clear();
    ///
    /// assert!(map.is_empty());
    /// ```
    #[inline(always)]
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl<T> SparseHashSet<T>
where
    T: Eq + Hash,
{
    /// Reserves capacity for at least additional more elements to be inserted in the SparseHashSet.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::with_capacity(5);
    ///
    /// # let mut map: SparseHashSet<()> = map;
    /// #
    /// assert!(map.capacity() >= 5);
    ///
    /// map.reserve(10);
    ///
    /// assert!(map.capacity() - map.len() >= 10);
    /// ```
    #[inline(always)]
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Tries to reserve capacity for at least additional more elements.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::with_capacity(5);
    ///
    /// # let mut map: SparseHashSet<()> = map;
    /// #
    /// assert!(map.capacity() >= 5);
    ///
    /// map.try_reserve(10).expect("Out of Memory!");
    ///
    /// assert!(map.capacity() - map.len() >= 10);
    /// ```
    #[inline(always)]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.0.try_reserve(additional)
    }

    /// Shrinks the capacity of the set as much as possible.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::with_capacity(5);
    ///
    /// map.insert("foo");
    /// map.insert("bar");
    /// map.insert("baz");
    ///
    /// assert!(map.capacity() >= 5);
    ///
    /// map.shrink_to_fit();
    ///
    /// assert_eq!(map.capacity(), 3);
    /// ```
    #[inline(always)]
    pub fn shrink_to_fit(&mut self) {
        self.0.shrink_to_fit();
    }

    /// Shrinks the capacity of the set with a lower limit.
    #[inline(always)]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.0.shrink_to(min_capacity);
    }

    /// Visits the values representing the difference
    #[inline(always)]
    pub fn difference<'a>(&'a self, other: &'a Self) -> Difference<'a, T, SparseHashState> {
        self.0.difference(&other.0)
    }

    /// Visits the values representing the symmetric difference
    #[inline(always)]
    pub fn symmetric_difference<'a>(
        &'a self,
        other: &'a Self,
    ) -> SymmetricDifference<'a, T, SparseHashState> {
        self.0.symmetric_difference(&other.0)
    }

    /// Visits the values representing the intersection
    #[inline(always)]
    pub fn intersection<'a>(&'a self, other: &'a Self) -> Intersection<'a, T, SparseHashState> {
        self.0.intersection(&other.0)
    }

    /// Visits the values representing the union
    #[inline(always)]
    pub fn union<'a>(&'a self, other: &'a Self) -> Union<'a, T, SparseHashState> {
        self.0.union(&other.0)
    }

    /// Returns true if the set contains a value.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    ///
    /// assert!(map.contains("foo"));
    /// ```
    #[inline(always)]
    pub fn contains<Q>(&self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T> + ?Sized,
    {
        self.0.contains(value)
    }

    /// Returns a reference to the value in the set, if any, that is equal to the given value.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    ///
    /// assert_eq!(map.get("foo"), Some(&"foo"));
    /// ```
    #[inline(always)]
    pub fn get<Q>(&self, value: &Q) -> Option<&T>
    where
        Q: Hash + Equivalent<T> + ?Sized,
    {
        self.0.get(value)
    }

    /// Inserts the given value into the set if it is not present,
    /// then returns a reference to the value in the set.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// assert_eq!(map.get_or_insert("foo"), &"foo");
    /// ```
    #[inline(always)]
    pub fn get_or_insert(&mut self, value: T) -> &T {
        self.0.get_or_insert(value)
    }

    /// Inserts a value computed from f into the set if the given value is not present,
    /// then returns a reference to the value in the set.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// assert_eq!(map.get_or_insert_with(&"foo", |_| "foo"), &"foo");
    /// ```
    #[inline(always)]
    pub fn get_or_insert_with<Q, F>(&mut self, value: &Q, f: F) -> &T
    where
        Q: Hash + Equivalent<T> + ?Sized,
        F: FnOnce(&Q) -> T,
    {
        self.0.get_or_insert_with(value, f)
    }

    /// Gets the given value's corresponding entry in the set for in-place manipulation.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// let value = map.entry("foo").or_insert();
    /// #
    /// # assert_eq!(value, ());
    /// ```
    #[inline(always)]
    pub fn entry(&mut self, value: T) -> hb::Entry<'_, T, SparseHashState> {
        self.0.entry(value)
    }

    /// Returns true if self has no elements in common with other.
    #[inline(always)]
    pub fn is_disjoint(&self, other: &Self) -> bool {
        self.0.is_disjoint(&other.0)
    }

    /// Returns true if the set is a subset of another
    #[inline(always)]
    pub fn is_subset(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    /// Returns true if the set is a superset of another
    #[inline(always)]
    pub fn is_superset(&self, other: &Self) -> bool {
        self.0.is_superset(&other.0)
    }

    /// Adds a value to the set.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    ///
    /// assert!(map.contains("foo"));
    /// ```
    #[inline(always)]
    pub fn insert(&mut self, value: T) -> bool {
        self.0.insert(value)
    }

    /// Adds a value to the set, replacing the existing value,
    /// if any, that is equal to the given one. Returns the replaced value.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    ///
    /// assert_eq!(map.replace("foo"), Some("foo"));
    /// ```
    #[inline(always)]
    pub fn replace(&mut self, value: T) -> Option<T> {
        self.0.replace(value)
    }

    /// Removes a value from the set. Returns whether the value was present in the set.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    ///
    /// assert!(map.remove("foo"));
    ///
    /// assert!(map.is_empty());
    /// ```
    #[inline(always)]
    pub fn remove<Q>(&mut self, value: &Q) -> bool
    where
        Q: Hash + Equivalent<T> + ?Sized,
    {
        self.0.remove(value)
    }

    /// Removes and returns the value in the set, if any, that is equal to the given one.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// map.insert("foo");
    ///
    /// assert_eq!(map.take("foo"), Some("foo"));
    ///
    /// assert!(map.is_empty());
    /// ```
    #[inline(always)]
    pub fn take<Q>(&mut self, value: &Q) -> Option<T>
    where
        Q: Hash + Equivalent<T> + ?Sized,
    {
        self.0.take(value)
    }

    /// Returns the total amount of memory allocated internally by the hash set, in bytes.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use voker_utils::hash::SparseHashSet;
    /// let mut map = SparseHashSet::new();
    ///
    /// assert_eq!(map.allocation_size(), 0);
    ///
    /// map.insert("foo");
    ///
    /// assert!(map.allocation_size() >= size_of::<&'static str>());
    /// ```
    #[inline(always)]
    pub fn allocation_size(&self) -> usize {
        self.0.allocation_size()
    }

    /// Insert a value the set without checking if the value already exists in the set.
    ///
    /// # Safety
    /// This operation is safe if a value does not exist in the set.
    ///
    /// However, if a value exists in the set already,
    /// the behavior is unspecified: this operation may panic, loop forever,
    /// or any following operation with the set may panic,
    /// loop forever or return arbitrary result.
    ///
    /// That said, this operation (and following operations)
    /// are guaranteed to not violate memory safety.
    ///
    /// However this operation is still unsafe
    /// because the resulting SparseHashSet may be passed to unsafe code
    /// which does expect the set to behave correctly,
    /// and would cause unsoundness as a result.
    #[expect(unsafe_code, reason = "re-exporting unsafe method")]
    #[inline(always)]
    pub unsafe fn insert_unique_unchecked(&mut self, value: T) -> &T {
        // SAFETY: safety contract is ensured by the caller.
        unsafe { self.0.insert_unique_unchecked(value) }
    }
}

impl<T> BitOr<&SparseHashSet<T>> for &SparseHashSet<T>
where
    for<'a> &'a InternalSet<T>: BitOr<&'a InternalSet<T>, Output = InternalSet<T>>,
{
    type Output = SparseHashSet<T>;

    /// Performs the | operation.
    #[inline(always)]
    fn bitor(self, rhs: &SparseHashSet<T>) -> SparseHashSet<T> {
        SparseHashSet(self.0.bitor(&rhs.0))
    }
}

impl<T> BitAnd<&SparseHashSet<T>> for &SparseHashSet<T>
where
    for<'a> &'a InternalSet<T>: BitAnd<&'a InternalSet<T>, Output = InternalSet<T>>,
{
    type Output = SparseHashSet<T>;

    /// Performs the & operation.
    #[inline(always)]
    fn bitand(self, rhs: &SparseHashSet<T>) -> SparseHashSet<T> {
        SparseHashSet(self.0.bitand(&rhs.0))
    }
}

impl<T> BitXor<&SparseHashSet<T>> for &SparseHashSet<T>
where
    for<'a> &'a InternalSet<T>: BitXor<&'a InternalSet<T>, Output = InternalSet<T>>,
{
    type Output = SparseHashSet<T>;

    /// Performs the ^ operation.
    #[inline(always)]
    fn bitxor(self, rhs: &SparseHashSet<T>) -> SparseHashSet<T> {
        SparseHashSet(self.0.bitxor(&rhs.0))
    }
}

impl<T> Sub<&SparseHashSet<T>> for &SparseHashSet<T>
where
    for<'a> &'a InternalSet<T>: Sub<&'a InternalSet<T>, Output = InternalSet<T>>,
{
    type Output = SparseHashSet<T>;

    /// Performs the - operation.
    #[inline(always)]
    fn sub(self, rhs: &SparseHashSet<T>) -> SparseHashSet<T> {
        SparseHashSet(self.0.sub(&rhs.0))
    }
}

impl<T> BitOrAssign<&SparseHashSet<T>> for SparseHashSet<T>
where
    InternalSet<T>: for<'a> BitOrAssign<&'a InternalSet<T>>,
{
    #[inline(always)]
    fn bitor_assign(&mut self, rhs: &SparseHashSet<T>) {
        self.0.bitor_assign(&rhs.0);
    }
}

impl<T> BitAndAssign<&SparseHashSet<T>> for SparseHashSet<T>
where
    InternalSet<T>: for<'a> BitAndAssign<&'a InternalSet<T>>,
{
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: &SparseHashSet<T>) {
        self.0.bitand_assign(&rhs.0);
    }
}

impl<T> BitXorAssign<&SparseHashSet<T>> for SparseHashSet<T>
where
    InternalSet<T>: for<'a> BitXorAssign<&'a InternalSet<T>>,
{
    #[inline(always)]
    fn bitxor_assign(&mut self, rhs: &SparseHashSet<T>) {
        self.0.bitxor_assign(&rhs.0);
    }
}

impl<T> SubAssign<&SparseHashSet<T>> for SparseHashSet<T>
where
    InternalSet<T>: for<'a> SubAssign<&'a InternalSet<T>>,
{
    #[inline(always)]
    fn sub_assign(&mut self, rhs: &SparseHashSet<T>) {
        self.0.sub_assign(&rhs.0);
    }
}
