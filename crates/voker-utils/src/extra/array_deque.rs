//! Fixed-capacity circular array buffer with stack storage.
//!
//! `ArrayDeque` stores elements in a fixed-size ring buffer without heap allocation.
//! It is useful for short-lived queues where capacity is known at compile time.
#![expect(unsafe_code, reason = "original implementation")]

use core::fmt;
use core::iter::FusedIterator;
use core::mem::MaybeUninit;
use core::ptr;
use core::slice;

// -----------------------------------------------------------------------------
// ArrayDeque

/// A ring buffer with fixed capacity, storing data on the stack.
///
/// `ArrayDeque` is a double-ended queue (deque) implemented as a circular buffer
/// with compile-time fixed capacity `N`. All data is stored in an array on the stack,
/// avoiding heap allocations.
///
/// Note that the back operation is faster than front.
///
/// # Examples
///
/// ```
/// use voker_utils::extra::ArrayDeque;
///
/// let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
///
/// // Push elements to the back
/// deque.push_back(1).unwrap();
/// deque.push_back(2).unwrap();
///
/// // Push elements to the front
/// deque.push_front(0).unwrap();
///
/// // Access elements
/// assert_eq!(deque.front(), Some(&0));
/// assert_eq!(deque.back(), Some(&2));
///
/// // Pop elements
/// assert_eq!(deque.pop_front(), Some(0));
/// assert_eq!(deque.pop_back(), Some(2));
///
/// // Check capacity constraints
/// deque.push_back(3).unwrap();
/// deque.push_back(4).unwrap();
/// deque.push_back(5).unwrap();
/// assert!(deque.is_full());
/// assert_eq!(deque.push_back(6), Err(6)); // Full, returns the element
/// ```
pub struct ArrayDeque<T, const N: usize> {
    slots: [MaybeUninit<T>; N],
    tail: usize,
    len: usize,
}

impl<T, const N: usize> Drop for ArrayDeque<T, N> {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T, const N: usize> Default for ArrayDeque<T, N> {
    /// Create an empty `ArrayDeque` with uninitialized backing storage.
    ///
    /// This uses `MaybeUninit` to avoid initializing the array elements.
    /// The returned queue has `len == 0` and `tail == 0`.
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> ArrayDeque<T, N> {
    #[inline]
    fn drop_inner(&mut self) {
        if self.len == N {
            unsafe {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut::<T>(
                    self.slots.as_mut_ptr() as *mut T,
                    N,
                ));
            }
            return;
        }
        let begin = (self.tail + N - self.len) % N;
        if self.tail > begin {
            unsafe {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut::<T>(
                    self.slots.as_mut_ptr().add(begin) as *mut T,
                    self.len,
                ));
            }
        } else {
            unsafe {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut::<T>(
                    self.slots.as_mut_ptr() as *mut T,
                    self.tail,
                ));
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut::<T>(
                    self.slots.as_mut_ptr().add(begin) as *mut T,
                    N - begin,
                ));
            }
        }
    }

    /// Returns a pair of slices containing the deque contents in front-to-back order.
    ///
    /// The returned slices represent the logical order of elements:
    ///
    /// - The first slice contains the first contiguous region.
    /// - The second slice contains the wrapped region, if any.
    ///
    /// When the internal storage is not wrapped, all elements are in the first slice
    /// and the second slice is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 5> = ArrayDeque::new();
    /// deque.push_back(0).unwrap();
    /// deque.push_back(1).unwrap();
    /// deque.push_back(2).unwrap();
    ///
    /// let expected = [0, 1, 2];
    /// let (front, back) = deque.as_slices();
    /// assert_eq!(&expected[..front.len()], front);
    /// assert_eq!(&expected[front.len()..], back);
    /// ```
    #[inline]
    pub fn as_slices(&self) -> (&[T], &[T]) {
        if self.len == 0 {
            return (&[], &[]);
        }

        let begin = (self.tail + N - self.len) % N;
        let ptr = self.slots.as_ptr() as *const T;

        if begin < self.tail {
            unsafe { (slice::from_raw_parts(ptr.add(begin), self.len), &[]) }
        } else {
            unsafe {
                (
                    slice::from_raw_parts(ptr.add(begin), N - begin),
                    slice::from_raw_parts(ptr, self.tail),
                )
            }
        }
    }

    /// Returns a pair of mutable slices containing the deque contents in front-to-back order.
    ///
    /// The two mutable slices are non-overlapping and together cover all elements.
    /// If the deque is not wrapped, the second slice is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
    /// deque.push_back(1).unwrap();
    /// deque.push_back(2).unwrap();
    /// deque.push_back(3).unwrap();
    ///
    /// let (front, back) = deque.as_mut_slices();
    /// for x in front {
    ///     *x *= 10;
    /// }
    /// for x in back {
    ///     *x *= 10;
    /// }
    ///
    /// assert_eq!(deque.pop_front(), Some(10));
    /// assert_eq!(deque.pop_front(), Some(20));
    /// assert_eq!(deque.pop_front(), Some(30));
    /// ```
    #[inline]
    pub fn as_mut_slices(&mut self) -> (&mut [T], &mut [T]) {
        if self.len == 0 {
            return (&mut [], &mut []);
        }

        let begin = (self.tail + N - self.len) % N;
        let ptr = self.slots.as_mut_ptr() as *mut T;

        if begin < self.tail {
            unsafe { (slice::from_raw_parts_mut(ptr.add(begin), self.len), &mut []) }
        } else {
            unsafe {
                (
                    slice::from_raw_parts_mut(ptr.add(begin), N - begin),
                    slice::from_raw_parts_mut(ptr, self.tail),
                )
            }
        }
    }

    /// Removes all elements from the deque.
    ///
    /// This method drops all elements currently in the deque and resets
    /// its internal state to empty. The capacity remains unchanged.
    pub fn clear(&mut self) {
        if core::mem::needs_drop::<T>() && self.len != 0 {
            self.drop_inner();
        }
        self.tail = 0;
        self.len = 0;
    }

    /// Creates an empty `ArrayDeque` with uninitialized backing storage.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let deque: ArrayDeque<i32, 10> = ArrayDeque::new();
    /// assert!(deque.is_empty());
    /// assert!(!deque.is_full());
    /// assert_eq!(deque.len(), 0);
    /// ```
    ///
    /// Note that the capacity `0` is valid.
    #[inline(always)]
    #[must_use]
    pub const fn new() -> Self {
        const {
            assert!(
                N <= (usize::MAX >> 2),
                "the capacity cannot exceed `usize::MAX / 4`"
            );
        }
        Self {
            slots: unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() },
            tail: 0,
            len: 0,
        }
    }

    /// Returns `true` if the buffer is full (len == capacity).
    ///
    /// When the deque is full, any attempt to push additional elements
    /// will fail, returning the element in an `Err`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 2> = ArrayDeque::new();
    /// assert!(!deque.is_full());
    ///
    /// deque.push_back(1).unwrap();
    /// deque.push_back(2).unwrap();
    /// assert!(deque.is_full());
    ///
    /// // Attempting to push another element will fail
    /// assert!(deque.push_back(3).is_err());
    /// ```
    #[inline(always)]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Returns `true` if the buffer is empty (len == 0).
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
    /// assert!(deque.is_empty());
    ///
    /// deque.push_back(1).unwrap();
    /// assert!(!deque.is_empty());
    /// ```
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of elements in the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
    /// assert_eq!(deque.len(), 0);
    ///
    /// deque.push_back(1).unwrap();
    /// deque.push_back(2).unwrap();
    /// assert_eq!(deque.len(), 2);
    /// ```
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns a reference to the front element, if present.
    ///
    /// This method does not remove the element from the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
    /// assert_eq!(deque.front(), None);
    ///
    /// deque.push_back(1).unwrap();
    /// deque.push_back(2).unwrap();
    /// assert_eq!(deque.front(), Some(&1));
    ///
    /// deque.push_front(0).unwrap();
    /// assert_eq!(deque.front(), Some(&0));
    /// ```
    pub const fn front(&self) -> Option<&T> {
        if !self.is_empty() {
            let front = (self.tail + N - self.len) % N;
            unsafe { Some(&*self.slots.as_ptr().add(front).cast::<T>()) }
        } else {
            None
        }
    }

    /// Returns a mutable reference to the front element, if present.
    ///
    /// This method does not remove the element from the deque.
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if !self.is_empty() {
            let front = (self.tail + N - self.len) % N;
            unsafe { Some(&mut *self.slots.as_mut_ptr().add(front).cast::<T>()) }
        } else {
            None
        }
    }

    /// Returns a reference to the back element, if present.
    ///
    /// This method does not remove the element from the deque.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
    /// assert_eq!(deque.back(), None);
    ///
    /// deque.push_back(1).unwrap();
    /// deque.push_back(2).unwrap();
    /// assert_eq!(deque.back(), Some(&2));
    ///
    /// deque.pop_back();
    /// assert_eq!(deque.back(), Some(&1));
    /// ```
    pub const fn back(&self) -> Option<&T> {
        if !self.is_empty() {
            let back = (self.tail + N - 1) % N;
            unsafe { Some(&*self.slots.as_ptr().add(back).cast::<T>()) }
        } else {
            None
        }
    }

    /// Returns a mutable reference to the back element, if present.
    ///
    /// This method does not remove the element from the deque.
    pub fn back_mut(&mut self) -> Option<&mut T> {
        if !self.is_empty() {
            let back = (self.tail + N - 1) % N;
            unsafe { Some(&mut *self.slots.as_mut_ptr().add(back).cast::<T>()) }
        } else {
            None
        }
    }

    /// Returns a reference to the element at logical index `index`.
    ///
    /// Index `0` corresponds to the front element.
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let (first, second) = self.as_slices();
        if index < first.len() {
            Some(&first[index])
        } else {
            Some(&second[index - first.len()])
        }
    }

    /// Returns a mutable reference to the element at logical index `index`.
    ///
    /// Index `0` corresponds to the front element.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        let (first, second) = self.as_mut_slices();
        if index < first.len() {
            Some(&mut first[index])
        } else {
            Some(&mut second[index - first.len()])
        }
    }

    /// Returns `true` if the deque contains an element equal to `value`.
    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        let (first, second) = self.as_slices();
        first.contains(value) || second.contains(value)
    }

    /// Returns an iterator over references to elements in front-to-back order.
    ///
    /// This is equivalent to iterating over `self.as_slices().0` then
    /// `self.as_slices().1`.
    pub fn iter(&self) -> Iter<'_, T> {
        let (first, second) = self.as_slices();
        Iter {
            inner: first.iter().chain(second.iter()),
        }
    }

    /// Returns an iterator over mutable references in front-to-back order.
    ///
    /// This is equivalent to iterating over `self.as_mut_slices().0` then
    /// `self.as_mut_slices().1`.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        let (first, second) = self.as_mut_slices();
        IterMut {
            inner: first.iter_mut().chain(second.iter_mut()),
        }
    }

    /// Pushes an element to the front of the deque without checking capacity.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `!self.is_full()`.
    /// Calling this method when the deque is full results in undefined behavior.
    #[inline(always)]
    pub const unsafe fn push_front_unchecked(&mut self, element: T) {
        let begin = (self.tail + (N << 1) - self.len - 1) % N;
        unsafe {
            ptr::write(self.slots.as_mut_ptr().add(begin) as *mut T, element);
        }
        self.len += 1;
    }

    /// Pushes an element to the front of the deque.
    ///
    /// Returns `Ok(())` if the element was inserted successfully.
    /// If the deque is full, returns `Err(element)` and leaves the deque unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 2> = ArrayDeque::new();
    ///
    /// assert_eq!(deque.push_front(1), Ok(()));
    /// assert_eq!(deque.push_front(0), Ok(()));
    /// assert_eq!(deque.front(), Some(&0));
    ///
    /// // Deque is now full
    /// assert_eq!(deque.push_front(-1), Err(-1));
    /// assert_eq!(deque.len(), 2); // Length unchanged
    /// ```
    #[inline]
    pub const fn push_front(&mut self, element: T) -> Result<(), T> {
        if !self.is_full() {
            unsafe {
                self.push_front_unchecked(element);
            }
            Ok(())
        } else {
            Err(element)
        }
    }

    /// Pushes an element to the back of the deque without checking capacity.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `!self.is_full()`.
    /// Calling this method when the deque is full results in undefined behavior.
    #[inline(always)]
    pub const unsafe fn push_back_unchecked(&mut self, element: T) {
        unsafe {
            ptr::write(self.slots.as_mut_ptr().add(self.tail) as *mut T, element);
        }
        self.tail = (self.tail + 1) % N;
        self.len += 1;
    }

    /// Pushes an element to the back of the deque.
    ///
    /// Returns `Ok(())` if the element was inserted successfully.
    /// If the deque is full, returns `Err(element)` and leaves the deque unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 2> = ArrayDeque::new();
    ///
    /// assert_eq!(deque.push_back(1), Ok(()));
    /// assert_eq!(deque.push_back(2), Ok(()));
    /// assert_eq!(deque.back(), Some(&2));
    ///
    /// // Deque is now full
    /// assert_eq!(deque.push_back(3), Err(3));
    /// assert_eq!(deque.len(), 2); // Length unchanged
    /// ```
    #[inline]
    pub const fn push_back(&mut self, element: T) -> Result<(), T> {
        if !self.is_full() {
            unsafe {
                self.push_back_unchecked(element);
            }
            Ok(())
        } else {
            Err(element)
        }
    }

    /// Removes and returns the front element of the deque.
    ///
    /// Returns `Some(T)` if the deque is not empty, otherwise returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
    /// deque.push_back(1).unwrap();
    /// deque.push_back(2).unwrap();
    ///
    /// assert_eq!(deque.pop_front(), Some(1));
    /// assert_eq!(deque.pop_front(), Some(2));
    /// assert_eq!(deque.pop_front(), None);
    /// ```
    #[inline]
    pub const fn pop_front(&mut self) -> Option<T> {
        if !self.is_empty() {
            let begin = (self.tail + N - self.len) % N;
            let value = unsafe { ptr::read(self.slots.as_mut_ptr().add(begin) as *mut T) };
            self.len -= 1;
            Some(value)
        } else {
            None
        }
    }

    /// Removes and returns the back element of the deque.
    ///
    /// Returns `Some(T)` if the deque is not empty, otherwise returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::ArrayDeque;
    ///
    /// let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
    /// deque.push_back(1).unwrap();
    /// deque.push_back(2).unwrap();
    ///
    /// assert_eq!(deque.pop_back(), Some(2));
    /// assert_eq!(deque.pop_back(), Some(1));
    /// assert_eq!(deque.pop_back(), None);
    /// ```
    #[inline]
    pub const fn pop_back(&mut self) -> Option<T> {
        if !self.is_empty() {
            self.tail = (self.tail + N - 1) % N;
            let value = unsafe { ptr::read(self.slots.as_mut_ptr().add(self.tail) as *mut T) };
            self.len -= 1;
            Some(value)
        } else {
            None
        }
    }
}

impl<T: Clone, const N: usize> Clone for ArrayDeque<T, N> {
    fn clone(&self) -> Self {
        let mut out = Self::new();
        for item in self.iter() {
            unsafe {
                out.push_back_unchecked(item.clone());
            }
        }
        out
    }
}

impl<T, const N: usize> From<[T; N]> for ArrayDeque<T, N> {
    fn from(value: [T; N]) -> Self {
        let mut deque = Self::new();
        for item in value {
            unsafe {
                deque.push_back_unchecked(item);
            }
        }
        deque
    }
}

impl<T, const N: usize> FromIterator<T> for ArrayDeque<T, N> {
    /// Builds a deque from an iterator.
    ///
    /// # Panics
    ///
    /// Panics if the iterator yields more than `N` items.
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut deque = Self::new();
        for item in iter {
            assert!(
                !deque.is_full(),
                "ArrayDeque::from_iter overflow: capacity is {N}"
            );
            unsafe {
                deque.push_back_unchecked(item);
            }
        }
        deque
    }
}

/// Shared iterator for `ArrayDeque`.
pub struct Iter<'a, T> {
    inner: core::iter::Chain<slice::Iter<'a, T>, slice::Iter<'a, T>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}
impl<T> FusedIterator for Iter<'_, T> {}

/// Mutable iterator for `ArrayDeque`.
pub struct IterMut<'a, T> {
    inner: core::iter::Chain<slice::IterMut<'a, T>, slice::IterMut<'a, T>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T> ExactSizeIterator for IterMut<'_, T> {}
impl<T> FusedIterator for IterMut<'_, T> {}

/// Owning iterator for `ArrayDeque`.
pub struct IntoIter<T, const N: usize> {
    deque: ArrayDeque<T, N>,
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.deque.pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.deque.len(), Some(self.deque.len()))
    }
}

impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {}
impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

impl<T, const N: usize> IntoIterator for ArrayDeque<T, N> {
    type Item = T;
    type IntoIter = IntoIter<T, N>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { deque: self }
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a ArrayDeque<T, N> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, const N: usize> IntoIterator for &'a mut ArrayDeque<T, N> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for ArrayDeque<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (first, second) = self.as_slices();
        f.debug_list()
            .entries(first.iter().chain(second.iter()))
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::ArrayDeque;

    #[test]
    fn is_sync_send() {
        use core::panic::{RefUnwindSafe, UnwindSafe};

        fn is_send<T: Send>() {}
        fn is_sync<T: Send>() {}
        fn is_unwindsafe<T: UnwindSafe>() {}
        fn is_refunwindsafe<T: RefUnwindSafe>() {}

        is_send::<ArrayDeque<i32, 0>>();
        is_sync::<ArrayDeque<i32, 0>>();
        is_unwindsafe::<ArrayDeque<i32, 0>>();
        is_refunwindsafe::<ArrayDeque<i32, 0>>();
    }

    #[test]
    fn drop_contiguous() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static DROPS: AtomicUsize = AtomicUsize::new(0);
        #[derive(Debug)]
        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        {
            let mut deque: ArrayDeque<Tracker, 8> = ArrayDeque::new();
            deque.push_back(Tracker).unwrap();
            deque.push_back(Tracker).unwrap();
            deque.push_back(Tracker).unwrap();

            assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn drop_full() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static DROPS: AtomicUsize = AtomicUsize::new(0);
        #[derive(Debug)]
        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        {
            let mut deque: ArrayDeque<Tracker, 4> = ArrayDeque::new();
            deque.push_back(Tracker).unwrap();
            deque.push_back(Tracker).unwrap();
            deque.push_back(Tracker).unwrap();
            deque.push_back(Tracker).unwrap();
            assert!(deque.is_full());

            assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn drop_wrapped() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static DROPS: AtomicUsize = AtomicUsize::new(0);
        #[derive(Debug)]
        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        {
            let mut deque: ArrayDeque<Tracker, 5> = ArrayDeque::new();
            deque.push_back(Tracker).unwrap();
            deque.push_back(Tracker).unwrap();
            deque.push_back(Tracker).unwrap();
            deque.push_back(Tracker).unwrap();

            let v1 = deque.pop_front().unwrap();
            let v2 = deque.pop_front().unwrap();
            drop(v1);
            drop(v2);
            assert_eq!(DROPS.load(Ordering::SeqCst), 2);

            // Make internal range wrap around while len < N.
            deque.push_back(Tracker).unwrap();

            assert_eq!(deque.len(), 3);
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), 5);
    }

    #[test]
    fn drop_clear_pop() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static DROPS: AtomicUsize = AtomicUsize::new(0);
        #[derive(Debug)]
        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        let mut deque: ArrayDeque<Tracker, 6> = ArrayDeque::new();
        deque.push_back(Tracker).unwrap();
        deque.push_back(Tracker).unwrap();
        deque.push_back(Tracker).unwrap();
        deque.push_back(Tracker).unwrap();

        let front = deque.pop_front().unwrap();
        assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        drop(front);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);

        deque.clear();
        assert!(deque.is_empty());
        assert_eq!(DROPS.load(Ordering::SeqCst), 4);

        drop(deque);
        assert_eq!(DROPS.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn mut_peek_and_contains() {
        let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
        deque.push_back(1).unwrap();
        deque.push_back(2).unwrap();

        assert!(deque.contains(&1));
        assert!(!deque.contains(&3));

        *deque.front_mut().unwrap() = 10;
        *deque.back_mut().unwrap() = 20;

        assert_eq!(deque.front(), Some(&10));
        assert_eq!(deque.back(), Some(&20));
    }

    #[test]
    fn get_and_get_mut() {
        let mut deque: ArrayDeque<i32, 5> = ArrayDeque::new();
        deque.push_back(1).unwrap();
        deque.push_back(2).unwrap();
        deque.push_back(3).unwrap();

        assert_eq!(deque.get(0), Some(&1));
        assert_eq!(deque.get(2), Some(&3));
        assert_eq!(deque.get(3), None);

        *deque.get_mut(1).unwrap() = 20;
        assert_eq!(deque.get(1), Some(&20));

        assert_eq!(deque.pop_front(), Some(1));
        deque.push_back(4).unwrap();
        deque.push_back(5).unwrap();
        assert_eq!(deque.get(0), Some(&20));
        assert_eq!(deque.get(3), Some(&5));
    }

    #[test]
    fn iter_is_fused() {
        let deque = ArrayDeque::<i32, 2>::from([1, 2]);
        let mut iter = deque.iter();
        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_mut_is_fused() {
        let mut deque = ArrayDeque::<i32, 2>::from([1, 2]);
        let mut iter = deque.iter_mut();
        assert_eq!(iter.next().map(|x| *x), Some(1));
        assert_eq!(iter.next().map(|x| *x), Some(2));
        assert_eq!(iter.next().map(|x| *x), None);
        assert_eq!(iter.next().map(|x| *x), None);
    }

    #[test]
    fn iter_wrap_order() {
        let mut deque: ArrayDeque<i32, 4> = ArrayDeque::new();
        deque.push_back(1).unwrap();
        deque.push_back(2).unwrap();
        deque.push_back(3).unwrap();

        assert_eq!(deque.pop_front(), Some(1));
        deque.push_back(4).unwrap();

        let got: Vec<_> = deque.iter().copied().collect();
        assert_eq!(got, [2, 3, 4]);
    }

    #[test]
    fn from_array_clone_and_into_iter() {
        let deque = ArrayDeque::<i32, 4>::from([1, 2, 3, 4]);
        assert_eq!(deque.len(), 4);

        let cloned = deque.clone();
        let got: Vec<_> = cloned.into_iter().collect();
        assert_eq!(got, [1, 2, 3, 4]);
    }

    #[test]
    #[should_panic(expected = "ArrayDeque::from_iter overflow")]
    fn from_iter_overflow_panics() {
        let _deque: ArrayDeque<i32, 2> = [1, 2, 3].into_iter().collect();
    }
}
