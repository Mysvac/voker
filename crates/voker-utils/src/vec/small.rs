use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::iter::FusedIterator;
use core::mem::ManuallyDrop;
use core::ptr;

use super::utils::min_cap;
use crate::cold_path;
use cache::Cache;

#[derive(Clone)]
enum InnerVec<T, const N: usize> {
    Cache(Cache<T, N>),
    Heap(Vec<T>),
}

/// A vector that stores up to `N` elements inline and spills to heap when needed.
///
/// This type combines stack-allocated fast-path storage (`N` items) with a `Vec` fallback
/// for larger workloads. Most methods mirror [`Vec`] semantics while preserving small-size
/// efficiency.
///
/// # Examples
///
/// ```
/// # use voker_utils::vec::SmallVec;
/// let mut vec: SmallVec<i32, 4> = SmallVec::new();
/// vec.extend([1, 2, 3, 4, 5]);
/// assert_eq!(vec, [1, 2, 3, 4, 5]);
/// ```
#[repr(transparent)]
pub struct SmallVec<T, const N: usize>(InnerVec<T, N>);

// -----------------------------------------------------------------------------
// Basic

impl<T, const N: usize> Default for SmallVec<T, N> {
    /// Constructs a new, empty [`SmallVec`] on the stack with the specified capacity.
    ///
    /// It's eq to [`SmallVec::new`] .
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone, const N: usize> Clone for SmallVec<T, N> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }

    fn clone_from(&mut self, source: &Self) {
        let src = source.as_slice();
        self.clear();
        self.extend(src);
    }
}

impl<T, const N: usize> SmallVec<T, N> {
    /// Constructs a new, empty [`SmallVec`] on the stack with the specified capacity.
    ///
    /// The capacity must be provided at compile time via the const generic parameter.
    ///
    /// Note that the stack memory is allocated when the `SmallVec` is instantiated.
    /// The capacity should not be too large to avoid stack overflow.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec: SmallVec<i32, 8> = SmallVec::new();
    /// ```
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self(InnerVec::Cache(Cache::new()))
    }

    /// Constructs a new, empty `SmallVec` with at least the specified capacity.
    ///
    /// If the specified capacity is less than or equal to `N`, this is equivalent
    /// to [`new`](SmallVec::new), and no heap memory will be allocated.
    #[must_use]
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity > N {
            Self(InnerVec::Heap(Vec::with_capacity(capacity)))
        } else {
            Self(InnerVec::Cache(Cache::new()))
        }
    }

    /// Reserves capacity for at least additional more elements to be inserted
    /// in the given `SmallVec`.
    ///
    /// The collection may reserve more space to speculatively avoid frequent reallocations.
    /// If the current capacity is sufficient, it will not be reallocated.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec: SmallVec<i32, 8> = SmallVec::new();
    /// assert!(vec.capacity() == 8);
    ///
    /// vec.reserve(10);
    /// assert!(vec.capacity() >= 10);
    /// ```
    pub fn reserve(&mut self, additional: usize) {
        let capacity = self.len() + additional;
        if capacity > N {
            match &mut self.0 {
                InnerVec::Cache(vec) => {
                    // SAFETY: capacity >= len
                    self.0 = InnerVec::Heap(unsafe { vec.move_to_vec_with_capacity(capacity) });
                }
                InnerVec::Heap(vec) => vec.reserve(additional),
            }
        } else {
            match &mut self.0 {
                InnerVec::Cache(_) => (),
                InnerVec::Heap(vec) => {
                    if capacity > vec.capacity() {
                        // SAFETY: capacity >= len && capacity <= N
                        self.0 = InnerVec::Cache(unsafe { Cache::from_vec_unchecked(vec) });
                    }
                }
            }
        }
    }

    /// Shrinks the capacity of the vector as much as possible.
    ///
    /// If the data is already in the stack, it won't do anything.
    ///
    /// If the capacity is sufficient, this function will move the data to stack.
    pub fn shrink_to_fit(&mut self) {
        match &mut self.0 {
            InnerVec::Heap(vec) => {
                if vec.len() > N {
                    vec.shrink_to_fit();
                } else {
                    // SAFETY: capacity >= len
                    self.0 = InnerVec::Cache(unsafe { Cache::from_vec_unchecked(vec) });
                }
            }
            InnerVec::Cache(_) => (),
        }
    }

    /// Copy elements from given slice.
    ///
    /// Because this function requires `Copy`, it's faster than `From/Into`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec = SmallVec::<i32, 8>::from_slice(&[1, 2, 3, 4, 5]);
    ///
    /// assert!(vec.capacity() == 8);
    /// assert_eq!(vec, [1, 2, 3, 4 ,5]);
    /// ```
    pub fn from_slice(slice: &[T]) -> Self
    where
        T: Copy,
    {
        if slice.len() <= N {
            let mut vec = Cache::new();
            unsafe {
                ptr::copy_nonoverlapping(slice.as_ptr(), vec.as_mut_ptr(), slice.len());
                vec.set_len(slice.len());
            }
            Self(InnerVec::Cache(vec))
        } else {
            cold_path();
            let mut vec = Vec::with_capacity(slice.len());
            unsafe {
                ptr::copy_nonoverlapping(slice.as_ptr(), vec.as_mut_ptr(), slice.len());
                vec.set_len(slice.len());
            }
            Self(InnerVec::Heap(vec))
        }
    }

    /// Returns a raw pointer to the vector’s buffer, or a dangling raw pointer
    /// valid for zero sized reads.
    #[inline]
    pub const fn as_ptr(&self) -> *const T {
        match &self.0 {
            InnerVec::Cache(vec) => vec.as_ptr(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.as_ptr()
            }
        }
    }

    /// Returns a raw pointer to the vector’s buffer, or a dangling raw pointer
    /// valid for zero sized reads.
    #[inline]
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        match &mut self.0 {
            InnerVec::Cache(vec) => vec.as_mut_ptr(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.as_mut_ptr()
            }
        }
    }

    /// Returns true if the vector contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec = SmallVec::<i32, 8>::new();
    /// assert!(vec.is_empty());
    ///
    /// vec.push(1);
    /// assert!(!vec.is_empty());
    /// ```
    #[inline]
    pub const fn is_empty(&self) -> bool {
        match &self.0 {
            InnerVec::Cache(vec) => vec.is_empty(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.is_empty()
            }
        }
    }

    /// Returns the number of elements in the vector, also referred to as its ‘length’.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec = SmallVec::<i32, 8>::new();
    /// assert_eq!(vec.len(), 0);
    ///
    /// vec.push(1);
    /// assert_eq!(vec.len(), 1);
    /// ```
    #[inline]
    pub const fn len(&self) -> usize {
        match &self.0 {
            InnerVec::Cache(vec) => vec.len(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.len()
            }
        }
    }

    /// Returns the total number of elements the vector can hold without reallocating.
    ///
    /// For `Cache`, this is always equal to `N` .
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec = SmallVec::<i32, 3>::new();
    /// assert_eq!(vec.capacity(), 3);
    ///
    /// vec.extend([1, 2, 3, 4]);
    /// assert!(vec.capacity() >= 4);
    /// ```
    #[inline]
    pub const fn capacity(&self) -> usize {
        match &self.0 {
            InnerVec::Cache(vec) => vec.capacity(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.capacity()
            }
        }
    }

    /// Forces the length of the vector to `new_len`.
    ///
    /// # Safety
    /// - `new_len` needs to be less than or equal to capacity `N`.
    /// - If the length is increased, it is necessary to ensure that the new element is initialized correctly.
    /// - If the length is reduced, it is necessary to ensure that the reduced elements can be dropped normally.
    ///
    /// See [`Vec::set_len`] for more infomation .
    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        // SAFETY: See function docs.
        unsafe {
            match &mut self.0 {
                InnerVec::Cache(vec) => vec.set_len(new_len),
                InnerVec::Heap(vec) => {
                    cold_path();
                    vec.set_len(new_len)
                }
            }
        }
    }

    /// Convert [`SmallVec`] to [`Vec`].
    ///
    /// If the data is in the stack, the exact memory will be allocated.
    /// If the data is in the heap, will not reallocate memory.
    ///
    /// Therefore, this function is efficient, but the returned [`Vec`] may not be tight.
    #[inline]
    pub fn into_vec(self) -> Vec<T> {
        match self.0 {
            InnerVec::Cache(mut vec) => vec.move_to_vec(),
            InnerVec::Heap(vec) => vec,
        }
    }

    /// Extracts a slice containing the entire vector.
    #[inline]
    pub const fn as_slice(&self) -> &[T] {
        match &self.0 {
            InnerVec::Cache(vec) => vec.as_slice(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.as_slice()
            }
        }
    }

    /// Extracts a mutable slice of the entire vector.
    #[inline]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        match &mut self.0 {
            InnerVec::Cache(vec) => vec.as_mut_slice(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.as_mut_slice()
            }
        }
    }

    /// Removes an element from the vector and returns it.
    ///
    /// The removed element is replaced by the last element of the vector.
    ///
    /// This function does not affect the position (stack/heap) of the data.
    ///
    /// # Panics
    ///
    /// Panics if `index` is out of bounds.
    #[inline]
    pub fn swap_remove(&mut self, index: usize) -> T {
        match &mut self.0 {
            InnerVec::Cache(vec) => vec.swap_remove(index),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.swap_remove(index)
            }
        }
    }

    /// Inserts an element at position `index` within the vector,
    /// shifting all elements after it to the right.
    ///
    /// If the heap is insufficient, it will switch to [`Vec`] and
    /// reserve some additional memory.
    ///
    /// # Panics
    /// Panics if `index > len`.
    ///
    /// # Examples
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec: SmallVec<_, 4> = ['a', 'b', 'c'].into();
    ///
    /// vec.insert(1, 'd');
    /// assert_eq!(vec, ['a', 'd', 'b', 'c']);
    ///
    /// vec.insert(4, 'e');
    /// assert_eq!(vec, ['a', 'd', 'b', 'c', 'e']);
    /// ```
    pub fn insert(&mut self, index: usize, element: T) {
        match &mut self.0 {
            InnerVec::Cache(vec) => {
                assert!(index <= vec.len(), "insertion index should be <= len");
                if !vec.is_full() {
                    // SAFETY: index <= len && len < N
                    unsafe {
                        vec.insert_unchecked(index, element);
                    }
                } else {
                    cold_path();
                    let mut new_vec: Vec<T> =
                        Vec::with_capacity(core::cmp::max(N << 1, min_cap::<T>()));
                    let dst_ptr = new_vec.as_mut_ptr();
                    let src_ptr = vec.as_ptr();
                    // SAFETY: enough capacity and valid data.
                    unsafe {
                        ptr::copy_nonoverlapping(src_ptr, dst_ptr, index);
                        ptr::write(dst_ptr.add(index), element);
                        ptr::copy_nonoverlapping(
                            src_ptr.add(index),
                            dst_ptr.add(index + 1),
                            N - index,
                        );
                        vec.set_len(0);
                        new_vec.set_len(N + 1);
                    }
                    self.0 = InnerVec::Heap(new_vec);
                }
            }
            InnerVec::Heap(vec) => {
                cold_path();
                vec.insert(index, element);
            }
        }
    }

    /// Removes and returns the element at position index within the
    /// vector, shifting all elements after it to the left.
    ///
    /// Note: Because this shifts over the remaining elements, it has a
    /// worst-case performance of O(n). If you don’t need the order of
    /// elements to be preserved, use [`swap_remove`](SmallVec::swap_remove)
    /// instead.
    ///
    /// This function does not affect the position (stack/heap) of the data.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    ///
    /// # Examples
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut v: SmallVec<_, 4> = ['a', 'b', 'c'].into();
    /// assert_eq!(v.remove(1), 'b');
    /// assert_eq!(v, ['a', 'c']);
    /// ```
    pub fn remove(&mut self, index: usize) -> T {
        match &mut self.0 {
            InnerVec::Cache(vec) => vec.remove(index),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.remove(index)
            }
        }
    }

    /// Appends an element to the back of a collection.
    ///
    /// If the heap is insufficient, it will switch to [`Vec`]
    /// and reserve some additional memory.
    ///
    /// # Time complexity
    /// Takes amortized O(1) time. If the vector’s length would
    /// exceed its capacity after the push, *O(capacity)* time is
    /// taken to copy the vector’s elements to a larger allocation.
    ///
    /// # Examples
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec: SmallVec<_, 4> = [1, 2].into();
    /// vec.push(3);
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    #[inline]
    pub fn push(&mut self, value: T) {
        match &mut self.0 {
            InnerVec::Cache(vec) => {
                if !vec.is_full() {
                    // SAFETY: len < N
                    unsafe { vec.push_unchecked(value) };
                } else {
                    cold_path();
                    let mut new_vec: Vec<T> =
                        Vec::with_capacity(core::cmp::max(N << 1, min_cap::<T>()));
                    let dst_ptr = new_vec.as_mut_ptr();
                    let src_ptr = vec.as_ptr();
                    // SAFETY: enough capacity and valid data.
                    unsafe {
                        ptr::copy_nonoverlapping(src_ptr, dst_ptr, N);
                        ptr::write(dst_ptr.add(N), value);
                        vec.set_len(0);
                        new_vec.set_len(N + 1);
                    }
                    self.0 = InnerVec::Heap(new_vec);
                }
            }
            InnerVec::Heap(vec) => {
                cold_path();
                vec.push(value);
            }
        }
    }

    /// Removes the last element from a vector and returns it,
    /// or `None` if it is empty.
    ///
    /// This function does not affect the position (stack/heap) of the data.
    ///
    /// # Time complexity
    /// O(1) time
    ///
    /// # Examples
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec: SmallVec<_, 4> = [1, 2, 3].into();
    /// assert_eq!(vec.pop(), Some(3));
    /// assert_eq!(vec, [1, 2]);
    /// ```
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        match &mut self.0 {
            InnerVec::Cache(vec) => vec.pop(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.pop()
            }
        }
    }

    /// Shortens the vector, keeping the first `len` elements and dropping the rest.
    ///
    /// If `len` is greater or equal to the vector’s current length, this has no effect.
    ///
    /// Note that this will not modify the capacity, so it will not move data.
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        match &mut self.0 {
            InnerVec::Cache(vec) => vec.truncate(len),
            InnerVec::Heap(vec) => vec.truncate(len),
        }
    }

    /// Clears the vector, removing all values.
    ///
    /// Note that this method has no effect on the allocated capacity of the vector.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut vec: SmallVec<_, 4> = [1, 2, 3].into();
    /// vec.clear();
    /// assert_eq!(vec, []);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        match &mut self.0 {
            InnerVec::Cache(vec) => vec.clear(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.clear();
            }
        }
    }
}

impl<T: PartialEq, const N: usize> SmallVec<T, N> {
    /// Removes consecutive repeated elements in the vector according
    /// to the PartialEq trait implementation.
    #[inline]
    pub fn dedup(&mut self) {
        match &mut self.0 {
            InnerVec::Cache(vec) => vec.dedup(),
            InnerVec::Heap(vec) => {
                cold_path();
                vec.dedup();
            }
        }
    }
}

super::utils::impl_commen_traits!(SmallVec<T, N>);

impl<T, const N: usize> Extend<T> for SmallVec<T, N> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (hint, _) = iter.size_hint();
        self.reserve(hint);
        for item in iter {
            self.push(item);
        }
    }
}

impl<'a, T: 'a + Clone, const N: usize> Extend<&'a T> for SmallVec<T, N> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        let (hint, _) = iter.size_hint();
        self.reserve(hint);

        for item in iter {
            self.push(item.clone());
        }
    }
}

impl<T, U, const N: usize> PartialEq<SmallVec<U, N>> for SmallVec<T, N>
where
    T: PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &SmallVec<U, N>) -> bool {
        PartialEq::eq(self.as_slice(), other.as_slice())
    }
}

// -----------------------------------------------------------------------------
// From/Into

impl<T: Clone, const N: usize> From<&[T]> for SmallVec<T, N> {
    fn from(value: &[T]) -> Self {
        if value.len() <= N {
            let mut vec = Cache::<T, N>::new();
            value.iter().for_each(|item| unsafe {
                vec.push_unchecked(item.clone());
            });
            SmallVec(InnerVec::Cache(vec))
        } else {
            SmallVec(InnerVec::Heap(Vec::from(value)))
        }
    }
}

impl<T: Clone, const N: usize> From<&mut [T]> for SmallVec<T, N> {
    #[inline]
    fn from(value: &mut [T]) -> Self {
        <Self as From<&[T]>>::from(value)
    }
}

impl<T, const N: usize> From<Vec<T>> for SmallVec<T, N> {
    #[inline]
    fn from(value: Vec<T>) -> Self {
        Self(InnerVec::Heap(value))
    }
}

impl<T, const N: usize> From<Box<[T]>> for SmallVec<T, N> {
    #[inline]
    fn from(value: Box<[T]>) -> Self {
        Self(InnerVec::Heap(Vec::from(value)))
    }
}

impl<T, const N: usize, const P: usize> From<[T; P]> for SmallVec<T, N> {
    #[inline]
    fn from(value: [T; P]) -> Self {
        if P <= N {
            let mut cache: Cache<T, N> = Cache::new();
            unsafe {
                let value = ManuallyDrop::new(value);
                ptr::copy_nonoverlapping(value.as_ptr(), cache.as_mut_ptr(), P);
                cache.set_len(P);
            }
            Self(InnerVec::Cache(cache))
        } else {
            Self(InnerVec::Heap(Vec::from(value)))
        }
    }
}

impl<T, const N: usize> From<SmallVec<T, N>> for Vec<T> {
    #[inline]
    fn from(value: SmallVec<T, N>) -> Self {
        match value.0 {
            InnerVec::Cache(mut vec) => vec.move_to_vec(),
            InnerVec::Heap(vec) => vec,
        }
    }
}

impl<T, const N: usize> From<SmallVec<T, N>> for Box<[T]> {
    #[inline]
    fn from(value: SmallVec<T, N>) -> Self {
        <Vec<T>>::from(value).into_boxed_slice()
    }
}

// -----------------------------------------------------------------------------
// Iterator

impl<T, const N: usize> FromIterator<T> for SmallVec<T, N> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (hint, _) = iter.size_hint();

        let mut vec = Self::with_capacity(hint);
        iter.for_each(|item| {
            vec.push(item);
        });

        vec
    }
}

impl<T, const N: usize> IntoIterator for SmallVec<T, N> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        match self.0 {
            InnerVec::Cache(vec) => IntoIter(InternalIter::Cache(vec.into_iter())),
            InnerVec::Heap(vec) => {
                cold_path();
                IntoIter(InternalIter::Heap(vec.into_iter()))
            }
        }
    }
}

// -----------------------------------------------------------------------------
// IntoIter

#[derive(Clone)]
enum InternalIter<T, const N: usize> {
    Cache(cache::IntoIter<T, N>),
    Heap(alloc::vec::IntoIter<T>),
}

/// An iterator that consumes a [`SmallVec`] and yields its items by value.
#[derive(Clone)]
#[repr(transparent)]
pub struct IntoIter<T, const N: usize>(InternalIter<T, N>);

impl<T, const N: usize> IntoIter<T, N> {
    /// Returns a slice of the remaining elements that have not been yielded yet.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut iter = SmallVec::<_, 4>::from([1, 2, 3]).into_iter();
    /// iter.next();
    /// assert_eq!(iter.as_slice(), &[2, 3]);
    /// ```
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        match &self.0 {
            InternalIter::Cache(iter) => iter.as_slice(),
            InternalIter::Heap(iter) => {
                cold_path();
                iter.as_slice()
            }
        }
    }

    /// Returns a mutable slice of the remaining elements that have not been yielded yet.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::vec::SmallVec;
    /// let mut iter = SmallVec::<_, 4>::from([1, 2, 3]).into_iter();
    /// iter.as_mut_slice()[0] = 10;
    /// assert_eq!(iter.next(), Some(10));
    /// ```
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        match &mut self.0 {
            InternalIter::Cache(iter) => iter.as_mut_slice(),
            InternalIter::Heap(iter) => {
                cold_path();
                iter.as_mut_slice()
            }
        }
    }
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            InternalIter::Cache(iter) => Iterator::next(iter),
            InternalIter::Heap(iter) => {
                cold_path();
                Iterator::next(iter)
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.0 {
            InternalIter::Cache(iter) => Iterator::size_hint(iter),
            InternalIter::Heap(iter) => {
                cold_path();
                Iterator::size_hint(iter)
            }
        }
    }
}

impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            InternalIter::Cache(iter) => DoubleEndedIterator::next_back(iter),
            InternalIter::Heap(iter) => {
                cold_path();
                DoubleEndedIterator::next_back(iter)
            }
        }
    }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {}

impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

impl<T: Debug, const N: usize> Debug for IntoIter<T, N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}

mod cache {
    use crate::cold_path;
    use crate::num::NonMaxUsize;
    use crate::vec::utils::*;
    use alloc::vec::Vec;
    use core::iter::FusedIterator;
    use core::mem::{ManuallyDrop, MaybeUninit};
    use core::{mem, ptr, slice};

    pub(super) struct Cache<T, const N: usize> {
        data: [MaybeUninit<T>; N],
        // We use `NonMax` to reduce 8 bytes in most usage.
        // Now the size of `SmallVec<u64, 4>` is `40` instead of `48`.
        len: NonMaxUsize,
    }

    impl<T, const N: usize> Drop for Cache<T, N> {
        fn drop(&mut self) {
            if mem::needs_drop::<T>() && !self.is_empty() {
                unsafe {
                    let to_drop = ptr::slice_from_raw_parts_mut(self.as_mut_ptr(), self.len());
                    ptr::drop_in_place(to_drop);
                }
            }
        }
    }

    impl<T, const N: usize> Cache<T, N> {
        #[inline]
        pub(super) const fn new() -> Self {
            const {
                assert!(
                    N <= (usize::MAX >> 1),
                    "the capacity cannot exceed `isize::MAX`",
                );
            }

            Self {
                // SAFETY: Full buffer uninitialized to internal uninitialized is safe.
                data: unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() },
                len: NonMaxUsize::ZERO,
            }
        }

        #[inline(always)]
        pub const fn as_ptr(&self) -> *const T {
            &raw const self.data as *const T
        }

        #[inline(always)]
        pub const fn as_mut_ptr(&mut self) -> *mut T {
            &raw mut self.data as *mut T
        }

        #[inline(always)]
        pub const unsafe fn set_len(&mut self, new_len: usize) {
            self.len = unsafe { NonMaxUsize::new_unchecked(new_len) };
        }

        #[inline(always)]
        pub const fn len(&self) -> usize {
            self.len.get()
        }

        #[inline(always)]
        pub const fn is_empty(&self) -> bool {
            unsafe { mem::transmute::<NonMaxUsize, usize>(self.len) == usize::MAX }
        }

        #[inline(always)]
        pub const fn is_full(&self) -> bool {
            unsafe { mem::transmute::<NonMaxUsize, usize>(self.len) == const { N ^ usize::MAX } }
        }

        #[inline(always)]
        pub const fn capacity(&self) -> usize {
            N
        }

        #[inline]
        pub fn move_to_vec(&mut self) -> Vec<T> {
            let length = self.len();
            let mut vec: Vec<T> = Vec::with_capacity(length);

            unsafe {
                ptr::copy_nonoverlapping(self.as_ptr(), vec.as_mut_ptr(), length);
                vec.set_len(length);
                self.set_len(0);
            }

            vec
        }

        #[inline(always)]
        pub unsafe fn move_to_vec_with_capacity(&mut self, capacity: usize) -> Vec<T> {
            let mut vec: Vec<T> = Vec::with_capacity(capacity);
            let length = self.len();
            unsafe {
                ptr::copy_nonoverlapping(self.as_ptr(), vec.as_mut_ptr(), length);
                vec.set_len(length);
                self.set_len(0);
            }

            vec
        }

        #[inline(always)]
        pub unsafe fn from_vec_unchecked(vec: &mut Vec<T>) -> Self {
            const {
                assert!(
                    N <= (usize::MAX >> 1),
                    "the capacity cannot exceed `isize::MAX`",
                );
            }

            let mut svec = Self::new();
            let length = vec.len();
            unsafe {
                ptr::copy_nonoverlapping(vec.as_ptr(), svec.as_mut_ptr(), length);
                svec.set_len(length);
                vec.set_len(0);
            }

            svec
        }

        #[inline(always)]
        pub const unsafe fn push_unchecked(&mut self, value: T) {
            let length: usize = self.len();

            if T::IS_ZST {
                mem::forget(value);
            } else {
                unsafe {
                    ptr::write(self.as_mut_ptr().add(length), value);
                }
            }

            unsafe {
                self.set_len(length + 1);
            }
        }

        #[inline]
        pub const unsafe fn insert_unchecked(&mut self, index: usize, element: T) {
            let length: usize = self.len();

            if T::IS_ZST {
                mem::forget(element);
            } else {
                unsafe {
                    let ptr = self.as_mut_ptr().add(index);
                    if index < length {
                        ptr::copy(ptr, ptr.add(1), length - index);
                    }
                    ptr::write(ptr, element);
                }
            }

            unsafe {
                self.set_len(length + 1);
            }
        }

        #[inline(always)]
        pub const fn pop(&mut self) -> Option<T> {
            if self.is_empty() {
                cold_path();
                None
            } else {
                unsafe {
                    let last = self.len() - 1;
                    self.set_len(last);
                    // This hint is provided to the caller of the `pop`, not the `pop` itself.
                    core::hint::assert_unchecked(self.len() < self.capacity());
                    if T::IS_ZST {
                        Some(zst_init())
                    } else {
                        Some(ptr::read(self.as_ptr().add(last)))
                    }
                }
            }
        }

        #[inline]
        pub const fn remove(&mut self, index: usize) -> T {
            let length: usize = self.len();
            assert!(index < length, "removal index should be < len");

            unsafe {
                let value: T;

                if T::IS_ZST {
                    value = zst_init();
                } else {
                    let ptr = self.as_mut_ptr().add(index);
                    value = ptr::read(ptr);
                    ptr::copy(ptr.add(1), ptr, length - index - 1);
                }

                self.set_len(length - 1);
                value
            }
        }

        #[inline]
        pub const fn swap_remove(&mut self, index: usize) -> T {
            let length: usize = self.len();
            assert!(index < length, "removal index should be < len");

            unsafe {
                let value: T;

                if T::IS_ZST {
                    value = zst_init();
                } else {
                    let base_ptr = self.as_mut_ptr();
                    value = ptr::read(base_ptr.add(index));
                    ptr::copy(base_ptr.add(length - 1), base_ptr.add(index), 1);
                }

                self.set_len(length - 1);
                value
            }
        }

        #[inline]
        pub fn truncate(&mut self, len: usize) {
            let length: usize = self.len();
            if length > len {
                unsafe {
                    ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                        self.as_mut_ptr().add(len),
                        length - len,
                    ));
                    self.set_len(len);
                }
            }
        }

        #[inline]
        pub fn clear(&mut self) {
            if !self.is_empty() {
                unsafe {
                    ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                        self.as_mut_ptr(),
                        self.len(),
                    ));
                    self.set_len(0);
                }
            }
        }

        #[inline]
        pub const fn as_slice(&self) -> &[T] {
            unsafe { slice::from_raw_parts(self.as_ptr(), self.len()) }
        }

        #[inline]
        pub const fn as_mut_slice(&mut self) -> &mut [T] {
            unsafe { slice::from_raw_parts_mut(self.as_mut_ptr(), self.len()) }
        }
    }

    impl<T: PartialEq, const N: usize> Cache<T, N> {
        #[inline]
        pub fn dedup(&mut self) {
            let length: usize = self.len();
            if length <= 1 {
                return;
            }

            let ptr = self.as_mut_ptr();
            let mut left = 0usize;

            unsafe {
                let mut p_l = ptr.add(left);
                for right in 1..length {
                    let p_r = ptr.add(right);
                    if PartialEq::ne(&*p_l, &*p_r) {
                        left += 1;
                        p_l = ptr.add(left);
                        if right != left {
                            ptr::swap(p_r, p_l);
                        }
                    }
                }
            }
            self.truncate(left + 1);
        }
    }

    impl<T: Clone, const N: usize> Clone for Cache<T, N> {
        fn clone(&self) -> Self {
            let mut vec = Self::new();
            for item in self.as_slice() {
                unsafe { vec.push_unchecked(item.clone()) };
            }
            vec
        }

        fn clone_from(&mut self, source: &Self) {
            self.clear();
            for item in source.as_slice() {
                unsafe { self.push_unchecked(item.clone()) };
            }
        }
    }

    // -----------------------------------------------------------------------------
    // IntoIter

    #[derive(Clone)]
    pub struct IntoIter<T, const N: usize> {
        vec: ManuallyDrop<Cache<T, N>>,
        index: usize,
    }

    unsafe impl<T, const N: usize> Send for IntoIter<T, N> where T: Send {}
    unsafe impl<T, const N: usize> Sync for IntoIter<T, N> where T: Sync {}

    impl<T, const N: usize> IntoIterator for Cache<T, N> {
        type Item = T;
        type IntoIter = IntoIter<T, N>;

        #[inline]
        fn into_iter(self) -> Self::IntoIter {
            IntoIter {
                vec: ManuallyDrop::new(self),
                index: 0,
            }
        }
    }

    impl<T, const N: usize> Iterator for IntoIter<T, N> {
        type Item = T;
        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if self.index < self.vec.len() {
                self.index += 1;
                if T::IS_ZST {
                    unsafe { Some(zst_init()) }
                } else {
                    unsafe { Some(ptr::read(self.vec.as_ptr().add(self.index - 1))) }
                }
            } else {
                None
            }
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            let v = self.vec.len() - self.index;
            (v, Some(v))
        }
    }

    impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
        #[inline]
        fn next_back(&mut self) -> Option<Self::Item> {
            let length: usize = self.vec.len();
            if self.index < length {
                unsafe {
                    self.vec.set_len(length - 1);
                }
                if T::IS_ZST {
                    unsafe { Some(zst_init()) }
                } else {
                    unsafe { Some(ptr::read(self.vec.as_ptr().add(length - 1))) }
                }
            } else {
                None
            }
        }
    }

    impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {
        #[inline]
        fn len(&self) -> usize {
            self.vec.len() - self.index
        }
    }

    impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

    impl<T, const N: usize> Drop for IntoIter<T, N> {
        fn drop(&mut self) {
            let length: usize = self.vec.len();
            if self.index < length {
                unsafe {
                    ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                        self.vec.as_mut_ptr().add(self.index),
                        length - self.index,
                    ));
                }
            }
        }
    }

    impl<T, const N: usize> IntoIter<T, N> {
        pub fn as_slice(&self) -> &[T] {
            let len = self.vec.len() - self.index;
            unsafe { slice::from_raw_parts(self.vec.as_ptr().add(self.index), len) }
        }

        pub fn as_mut_slice(&mut self) -> &mut [T] {
            let len = self.vec.len() - self.index;
            unsafe { slice::from_raw_parts_mut(self.vec.as_mut_ptr().add(self.index), len) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SmallVec;
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn drop_cache() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);
        {
            let mut vec = SmallVec::<Tracker, 4>::new();
            vec.push(Tracker);
            vec.push(Tracker);
            vec.push(Tracker);

            assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        }
        assert_eq!(DROPS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn drop_heap() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);
        {
            let mut vec = SmallVec::<Tracker, 2>::new();
            vec.push(Tracker);
            vec.push(Tracker);
            vec.push(Tracker);
            assert!(vec.capacity() >= 3);

            assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        }
        assert_eq!(DROPS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn drop_pop_remove() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        let mut vec = SmallVec::<Tracker, 4>::new();
        vec.push(Tracker);
        vec.push(Tracker);
        vec.push(Tracker);

        let popped = vec.pop().unwrap();
        assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        drop(popped);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);

        let removed = vec.remove(0);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);
        drop(removed);
        assert_eq!(DROPS.load(Ordering::SeqCst), 2);

        drop(vec);
        assert_eq!(DROPS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn drop_into_iter_cache() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        let mut vec = SmallVec::<Tracker, 4>::new();
        vec.push(Tracker);
        vec.push(Tracker);
        vec.push(Tracker);

        let mut iter = vec.into_iter();
        let first = iter.next().unwrap();
        drop(first);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);

        drop(iter);
        assert_eq!(DROPS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn drop_into_iter_heap() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        let mut vec = SmallVec::<Tracker, 2>::new();
        vec.push(Tracker);
        vec.push(Tracker);
        vec.push(Tracker);
        vec.push(Tracker);

        let mut iter = vec.into_iter();
        let back = iter.next_back().unwrap();
        drop(back);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);

        drop(iter);
        assert_eq!(DROPS.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn drop_transition() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        let mut vec = SmallVec::<Tracker, 3>::new();
        vec.push(Tracker);
        vec.push(Tracker);
        vec.push(Tracker);

        vec.reserve(1);
        assert!(vec.capacity() >= 4);
        assert_eq!(DROPS.load(Ordering::SeqCst), 0);

        vec.truncate(2);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);

        vec.shrink_to_fit();
        assert_eq!(vec.capacity(), 3);

        drop(vec);
        assert_eq!(DROPS.load(Ordering::SeqCst), 3);
    }
}
