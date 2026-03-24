//! Provide block based linked-list.
//!
//! Improve cache hit rate through block data.
#![expect(unsafe_code, reason = "original implementation")]

use alloc::boxed::Box;
use core::fmt;
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr;

use crate::vec::ArrayVec;

// -----------------------------------------------------------------------------
// Config

const BLOCK_SIZE: usize = 13;

const MAX_IDLE: usize = 4;

// -----------------------------------------------------------------------------
// Block

/// A single queue block.
struct Block<T> {
    /// the index of head.
    ///
    /// For example the buffer is `[0, 0, 1, 1, 0]`
    /// (`0` indicates no data), then this index is `2`.
    head: usize,

    /// the index of tail.
    ///
    /// For example the buffer is `[0, 0, 1, 1, 0]`
    /// (`0` indicates no data), then this index is `4`.
    tail: usize,

    data: [MaybeUninit<T>; BLOCK_SIZE],
    next: *mut Block<T>,
}

impl<T> Block<T> {
    /// Create a empty block.
    #[cold]
    fn new() -> Box<Self> {
        Box::new(
            const {
                Block::<T> {
                    head: 0,
                    tail: 0,
                    // SAFETY: Convert full uninit to internal uninit is safe.
                    data: unsafe {
                        <MaybeUninit<[MaybeUninit<T>; BLOCK_SIZE]>>::uninit().assume_init()
                    },
                    next: ptr::null_mut(),
                }
            },
        )
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.next = ptr::null_mut();
    }
}

/// Drop remaining initialized elements in a block.
///
/// Only elements in range [head_index, tail_index) are valid.
impl<T> Drop for Block<T> {
    fn drop(&mut self) {
        if !core::mem::needs_drop::<T>() {
            return;
        }
        if self.head < self.tail {
            unsafe {
                ptr::drop_in_place(ptr::slice_from_raw_parts_mut::<T>(
                    self.data.as_mut_ptr().add(self.head) as *mut T,
                    self.tail - self.head,
                ));
            }
        }
    }
}

// -----------------------------------------------------------------------------
// BlockList

/// A queue implemented as a linked list of fixed-size blocks.
///
/// `BlockList` provides an efficient queue implementation that:
///
/// - Allocates memory in fixed-size blocks (13 elements per block)
/// - Recycles fully popped blocks to avoid frequent allocations
/// - Maintains a small pool of idle blocks (up to 4) for reuse
///
/// # Examples
///
/// ```
/// # use voker_utils::extra::BlockList;
/// let mut queue = BlockList::new();
/// assert!(queue.is_empty());
///
/// queue.push_back(1);
/// queue.push_back(2);
///
/// assert_eq!(queue.pop_front(), Some(1));
/// assert_eq!(queue.len(), 1);
///
/// assert_eq!(queue.pop_front(), Some(2));
/// assert_eq!(queue.pop_front(), None);
/// ```
pub struct BlockList<T> {
    head_ptr: *mut Block<T>,
    tail_ptr: *mut Block<T>,
    block_num: usize,
    idle: ArrayVec<Box<Block<T>>, MAX_IDLE>,
    _marker: PhantomData<T>,
}

unsafe impl<T: Sync> Sync for BlockList<T> {}
unsafe impl<T: Send> Send for BlockList<T> {}
impl<T: UnwindSafe> UnwindSafe for BlockList<T> {}
impl<T: RefUnwindSafe> RefUnwindSafe for BlockList<T> {}

impl<T> Drop for BlockList<T> {
    fn drop(&mut self) {
        let mut ptr = self.head_ptr;
        while !ptr.is_null() {
            unsafe {
                let boxed = Box::from_raw(ptr);
                ptr = (*ptr).next;
                ::core::mem::drop(boxed);
            }
        }
    }
}

impl<T> BlockList<T> {
    #[inline]
    fn get_block(&mut self) -> *mut Block<T> {
        if let Some(mut boxed) = self.idle.pop() {
            boxed.reset();
            Box::leak(boxed)
        } else {
            Box::leak(<Block<T>>::new())
        }
    }

    #[inline]
    fn idle_block(&mut self, ptr: *mut Block<T>) {
        // SAFERT: valid ptr, created through `Box::new`.
        let boxed = unsafe { Box::from_raw(ptr) };
        if !self.idle.is_full() {
            // SAFETY: !is_full()
            unsafe {
                self.idle.push_unchecked(boxed);
            }
        }
    }

    /// Creates an empty `BlockList`.
    ///
    /// This function does not allocate any memory.
    /// The first allocation occurs when the first element is pushed.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_utils::extra::BlockList;
    ///
    /// let queue: BlockList<i32> = BlockList::new();
    /// assert!(queue.is_empty());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            head_ptr: ptr::null_mut(),
            tail_ptr: ptr::null_mut(),
            block_num: 0,
            idle: ArrayVec::new(),
            _marker: PhantomData,
        }
    }

    /// Create a non-idle block , set head_ptr and tail_ptr.
    ///
    /// # Safety
    ///
    /// Self is uninit (head_ptr and tail_ptr is null).
    #[cold]
    #[inline(never)]
    fn init(&mut self) {
        debug_assert!(self.head_ptr.is_null());
        debug_assert!(self.tail_ptr.is_null());
        debug_assert_eq!(self.block_num, 0);
        let ptr = self.get_block();
        self.head_ptr = ptr;
        self.tail_ptr = ptr;
    }

    /// Appends an element to the back of the queue.
    ///
    /// If the current tail block is full, a new block will be allocated
    /// (or reused from the idle pool) and linked to the queue.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BlockList;
    /// let mut queue = BlockList::new();
    ///
    /// queue.push_back(1);
    /// queue.push_back(2);
    /// assert_eq!(queue.len(), 2);
    /// ```
    pub fn push_back(&mut self, value: T) {
        if self.tail_ptr.is_null() {
            self.init();
        }

        // SAFETY: `tail_ptr` point to valid data.
        let block = unsafe { &mut *self.tail_ptr };

        let index = block.tail;
        debug_assert!(index < BLOCK_SIZE);

        // SAFETY: valid index and pointer
        unsafe {
            ptr::write(block.data.as_mut_ptr().add(index) as *mut T, value);
        }

        block.tail = index + 1;

        if block.tail == BLOCK_SIZE {
            let new_block = self.get_block();
            block.next = new_block;

            self.tail_ptr = new_block;
            self.block_num += 1;
        }
    }

    /// Removes and returns the element from the front of the queue.
    ///
    /// Returns `None` if the queue is empty.
    /// If a block becomes empty after popping, it is moved to the idle pool
    /// for potential reuse (up to `MAX_IDLE` blocks).
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BlockList;
    /// let mut queue = BlockList::new();
    ///
    /// queue.push_back(1);
    /// queue.push_back(2);
    /// assert_eq!(queue.pop_front(), Some(1));
    /// assert_eq!(queue.pop_front(), Some(2));
    /// assert_eq!(queue.pop_front(), None);
    /// ```
    pub fn pop_front(&mut self) -> Option<T> {
        if self.head_ptr.is_null() {
            return None;
        }

        // SAFETY: `guard.0` point to valid data.
        let block = unsafe { &mut *self.head_ptr };
        let index = block.head;
        debug_assert!(index < BLOCK_SIZE);
        debug_assert!(index <= block.tail);

        if index == block.tail {
            return None;
        }

        // SAFETY: valid index and pointer
        let value = unsafe { ptr::read(block.data.as_ptr().add(index) as *mut T) };

        // SAFETY: `Block::drop` based on head and tail.
        // Must change `head` before `idle_block`.
        block.head = index + 1;

        if block.head == BLOCK_SIZE {
            let old_ptr = block as *mut Block<T>;
            let next_ptr = block.next;
            // index + 1 == BLOCK_SIZE, so tail_index == BLOCK_SIZE.
            // next_ptr must be set by `push` function.
            debug_assert!(!next_ptr.is_null());
            self.head_ptr = next_ptr;
            self.block_num -= 1;
            self.idle_block(old_ptr);
        }
        Some(value)
    }

    /// Returns a reference to the front element without removing it.
    pub fn front(&self) -> Option<&T> {
        if self.head_ptr.is_null() {
            return None;
        }

        let block = unsafe { &*self.head_ptr };
        if block.head == block.tail {
            return None;
        }

        unsafe { Some(&*block.data.as_ptr().add(block.head).cast::<T>()) }
    }

    /// Returns a mutable reference to the front element without removing it.
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if self.head_ptr.is_null() {
            return None;
        }

        let block = unsafe { &mut *self.head_ptr };
        if block.head == block.tail {
            return None;
        }

        unsafe { Some(&mut *block.data.as_mut_ptr().add(block.head).cast::<T>()) }
    }

    /// Returns `true` if the queue contains no elements.
    ///
    /// O(1) time complexity.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BlockList;
    /// let mut queue = BlockList::new();
    ///
    /// assert!(queue.is_empty());
    /// queue.push_back(1);
    /// assert!(!queue.is_empty());
    /// queue.pop_front();
    /// assert!(queue.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        if self.head_ptr.is_null() {
            return true;
        }
        let block = unsafe { &*self.head_ptr };
        block.tail == block.head
    }

    /// Returns the number of elements in the queue.
    ///
    /// O(1) time complexity.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BlockList;
    /// let mut queue = BlockList::new();
    ///
    /// queue.push_back(1);
    /// queue.push_back(2);
    /// queue.push_back(3);
    /// assert_eq!(queue.len(), 3);
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        if self.head_ptr.is_null() {
            return 0;
        }
        debug_assert!(!self.tail_ptr.is_null());
        let head_index = unsafe { (*self.head_ptr).head };
        let tail_index = unsafe { (*self.tail_ptr).tail };
        self.block_num * BLOCK_SIZE + tail_index - head_index
    }

    /// Returns an iterator over shared references in queue order.
    pub fn iter(&self) -> Iter<'_, T> {
        if self.head_ptr.is_null() {
            return Iter {
                block: ptr::null(),
                index: 0,
                tail: ptr::null(),
                len: 0,
                _marker: PhantomData,
            };
        }
        let head = unsafe { &*self.head_ptr };
        Iter {
            block: self.head_ptr,
            index: head.head,
            tail: self.tail_ptr,
            len: self.len(),
            _marker: PhantomData,
        }
    }

    /// Returns an iterator over mutable references in queue order.
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        if self.head_ptr.is_null() {
            return IterMut {
                block: ptr::null_mut(),
                index: 0,
                tail: ptr::null_mut(),
                len: 0,
                _marker: PhantomData,
            };
        }
        let head = unsafe { &*self.head_ptr };
        IterMut {
            block: self.head_ptr,
            index: head.head,
            tail: self.tail_ptr,
            len: self.len(),
            _marker: PhantomData,
        }
    }

    /// Clears the queue, removing all values.
    ///
    /// After calling `clear`, the queue will be empty.
    /// Blocks that become empty are moved to the idle pool for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_utils::extra::BlockList;
    /// let mut queue = BlockList::new();
    ///
    /// queue.push_back(1);
    /// queue.push_back(2);
    ///
    /// queue.clear();
    /// assert!(queue.is_empty());
    /// assert_eq!(queue.len(), 0);
    /// ```
    #[inline]
    pub fn clear(&mut self) {
        loop {
            if self.pop_front().is_none() {
                return;
            }
        }
    }
}

impl<T> Default for BlockList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> Clone for BlockList<T> {
    fn clone(&self) -> Self {
        let mut out = Self::new();
        for item in self.iter() {
            out.push_back(item.clone());
        }
        out
    }
}

impl<T> FromIterator<T> for BlockList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut out = Self::new();
        for item in iter {
            out.push_back(item);
        }
        out
    }
}

impl<T, const N: usize> From<[T; N]> for BlockList<T> {
    fn from(value: [T; N]) -> Self {
        value.into_iter().collect()
    }
}

/// Shared iterator for [`BlockList`].
pub struct Iter<'a, T> {
    block: *const Block<T>,
    index: usize,
    tail: *const Block<T>,
    len: usize,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.block.is_null() {
            let block = unsafe { &*self.block };
            if self.index < block.tail {
                let index = self.index;
                self.index += 1;
                self.len -= 1;
                return unsafe { Some(&*block.data.as_ptr().add(index).cast::<T>()) };
            }
            if ptr::eq(self.block, self.tail) {
                self.block = ptr::null();
                break;
            }
            self.block = block.next;
            self.index = 0;
        }
        debug_assert_eq!(self.len, 0);
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}
impl<T> FusedIterator for Iter<'_, T> {}

/// Mutable iterator for [`BlockList`].
pub struct IterMut<'a, T> {
    block: *mut Block<T>,
    index: usize,
    tail: *mut Block<T>,
    len: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.block.is_null() {
            let block = unsafe { &mut *self.block };
            if self.index < block.tail {
                let index = self.index;
                self.index += 1;
                self.len -= 1;
                return unsafe { Some(&mut *block.data.as_mut_ptr().add(index).cast::<T>()) };
            }
            if ptr::eq(self.block, self.tail) {
                self.block = ptr::null_mut();
                break;
            }
            self.block = block.next;
            self.index = 0;
        }
        debug_assert_eq!(self.len, 0);
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T> ExactSizeIterator for IterMut<'_, T> {}
impl<T> FusedIterator for IterMut<'_, T> {}

/// Owning iterator for [`BlockList`].
pub struct IntoIter<T> {
    list: BlockList<T>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.list.pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.list.len();
        (len, Some(len))
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}
impl<T> FusedIterator for IntoIter<T> {}

impl<T> IntoIterator for BlockList<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { list: self }
    }
}

impl<'a, T> IntoIterator for &'a BlockList<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut BlockList<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T> fmt::Debug for BlockList<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BlockList")
            .field("len", &self.len())
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{BLOCK_SIZE, BlockList};
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn is_sync_send() {
        use core::panic::{RefUnwindSafe, UnwindSafe};

        fn is_send<T: Send>() {}
        fn is_sync<T: Send>() {}
        fn is_unwindsafe<T: UnwindSafe>() {}
        fn is_refunwindsafe<T: RefUnwindSafe>() {}

        is_send::<BlockList<i32>>();
        is_sync::<BlockList<i32>>();
        is_unwindsafe::<BlockList<i32>>();
        is_refunwindsafe::<BlockList<i32>>();
    }

    #[test]
    fn drop_list() {
        use core::sync::atomic::{AtomicUsize, Ordering};

        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        {
            let mut q = BlockList::<Tracker>::new();
            for _ in 0..(BLOCK_SIZE * 2 + 5) {
                q.push_back(Tracker);
            }
            assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        }

        assert_eq!(DROPS.load(Ordering::SeqCst), BLOCK_SIZE * 2 + 5);
    }

    #[test]
    fn drop_pop() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        let mut q = BlockList::<Tracker>::new();
        q.push_back(Tracker);
        q.push_back(Tracker);
        q.push_back(Tracker);

        let first = q.pop_front().unwrap();
        assert_eq!(DROPS.load(Ordering::SeqCst), 0);
        drop(first);
        assert_eq!(DROPS.load(Ordering::SeqCst), 1);

        drop(q);
        assert_eq!(DROPS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn drop_clear() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        let mut q = BlockList::<Tracker>::new();
        for _ in 0..(BLOCK_SIZE + 3) {
            q.push_back(Tracker);
        }

        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        assert_eq!(DROPS.load(Ordering::SeqCst), BLOCK_SIZE + 3);

        drop(q);
        assert_eq!(DROPS.load(Ordering::SeqCst), BLOCK_SIZE + 3);
    }

    #[test]
    fn drop_with_idle_blocks() {
        static DROPS: AtomicUsize = AtomicUsize::new(0);

        struct Tracker;
        impl Drop for Tracker {
            fn drop(&mut self) {
                DROPS.fetch_add(1, Ordering::SeqCst);
            }
        }

        DROPS.store(0, Ordering::SeqCst);

        {
            let mut q = BlockList::<Tracker>::new();
            for _ in 0..(BLOCK_SIZE * 2) {
                q.push_back(Tracker);
            }

            // Drain exactly one full block so it enters idle pool.
            for _ in 0..BLOCK_SIZE {
                let v = q.pop_front().unwrap();
                drop(v);
            }

            assert_eq!(q.len(), BLOCK_SIZE);
            assert_eq!(DROPS.load(Ordering::SeqCst), BLOCK_SIZE);
        }

        // Remaining live elements in queue are dropped once; idle blocks are empty and should not add drops.
        assert_eq!(DROPS.load(Ordering::SeqCst), BLOCK_SIZE * 2);
    }

    #[test]
    fn front_and_front_mut() {
        let mut q = BlockList::new();
        assert_eq!(q.front(), None);
        assert_eq!(q.front_mut(), None);

        q.push_back(1);
        q.push_back(2);
        q.push_back(3);
        assert_eq!(q.front(), Some(&1));

        *q.front_mut().unwrap() = 10;
        assert_eq!(q.front(), Some(&10));

        assert_eq!(q.pop_front(), Some(10));
        assert_eq!(q.front(), Some(&2));
    }

    #[test]
    fn iter_and_iter_mut_order() {
        let mut q = BlockList::new();
        for i in 0..(BLOCK_SIZE + 3) {
            q.push_back(i as i32);
        }

        let before: Vec<_> = q.iter().copied().collect();
        assert_eq!(before.len(), BLOCK_SIZE + 3);
        assert_eq!(before[0], 0);

        for x in q.iter_mut() {
            *x += 1;
        }

        let after: Vec<_> = (&q).into_iter().copied().collect();
        assert_eq!(after[0], 1);
        assert_eq!(after[BLOCK_SIZE + 2], (BLOCK_SIZE + 3) as i32);
    }

    #[test]
    fn clone_from_iter_and_into_iter() {
        let q: BlockList<i32> = (0..20).collect();
        let cloned = q.clone();

        let got: Vec<_> = cloned.into_iter().collect();
        assert_eq!(got.len(), 20);
        assert_eq!(got[0], 0);
        assert_eq!(got[19], 19);

        let q2 = BlockList::from([10, 11, 12]);
        let got2: Vec<_> = q2.into_iter().collect();
        assert_eq!(got2, [10, 11, 12]);
    }
}
