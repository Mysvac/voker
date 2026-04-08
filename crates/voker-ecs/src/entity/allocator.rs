//! This module provides the implementation of the entity allocator.
//!
//! # Overview
//!
//! - Valid entity IDs range from `[1, u32::MAX - 1]`. New entities are allocated sequentially
//!   starting from 1.
//! - Entities support reuse. When allocating, previously recycled entities are prioritized,
//!   though their allocation order may be non-deterministic.
//! - Entity recycling requires mutable access (`&mut self`).
//! - Entity allocation is thread-safe, supporting concurrent allocation from multiple threads,
//!   including allocations without direct `World` access.
//! - Entity allocation and recycling do not modify the entity's `Generation` field.
//!   This must be managed separately by the caller.
//!
//! # Implementation Details
//!
//! The core of entity allocation is the thread-safe [`SharedAllocator`], which consists of three components:
//! - [`FreshAllocator`]: Allocates new (never-before-used) entities.
//! - [`FreeList`]: Manages a collection of recycled entities available for reuse.
//! - [`is_closed`](AtomicBool): An atomic flag used to ensure concurrent safety.
//!
//! The `FreshAllocator` is straightforward: since new entities are always allocated sequentially,
//! it only needs a `u32` counter to track the next available entity ID.
//!
//! The `FreeList` is conceptually similar to `Mutex<Vec<Entity>>` but with several optimizations:
//!   - It consists of [`FreeBuffer`] and [`FreeCount`] fields.
//!   - `FreeBuffer` is a chunked table (similar to a 2D array) that allocates memory lazily,
//!     avoiding the full-copy overhead of `Vec` resizing.
//!   - `FreeCount` is an atomic packed state that tracks the number of available entities
//!     and includes generation counters and disable flags for synchronization.
//!
//! To support remote allocation (allocation without direct `World` access), two interfaces are provided:
//! - [`EntityAllocator`]: Bound to a `World` instance, used for normal in-world allocations.
//! - [`RemoteAllocator`]: Provides allocation capabilities without requiring `World` access.
//!
//! Both are thin wrappers around [`SharedAllocator`].

use alloc::boxed::Box;
use core::fmt::Debug;
use core::iter::FusedIterator;
use core::mem::MaybeUninit;
use core::ptr;
use core::slice;
use core::sync::atomic::Ordering;

use voker_os::sync::Arc;
use voker_os::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, AtomicU64};
use voker_os::utils::Backoff;
use voker_utils::vec::ArrayVec;

use crate::entity::Entity;
use crate::entity::EntityId;
use crate::entity::EntityTag;
use crate::utils::DebugCheckedUnwrap;

// -----------------------------------------------------------------------------
// Chunk

/// An atomic pointer to an array of `Entity` values.
/// Length information is managed by the parent [`FreeBuffer`].
struct Chunk {
    head: AtomicPtr<Entity>,
}

impl Chunk {
    /// Constructs an empty [`Chunk`] with a null pointer.
    const fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    /// Allocates memory for the chunk.
    ///
    /// # Safety
    /// - The caller must ensure concurrency safety.
    /// - This should only be called when the chunk is uninitialized (head is null).
    #[cold]
    #[inline(never)]
    unsafe fn alloc(&self, capacity: u32) -> *mut Entity {
        let len = capacity as usize;
        // Using `new_uninit` is faster than `new_zeroed` for uninitialized allocation.
        let mut boxed: Box<[MaybeUninit<Entity>]> = Box::new_uninit_slice(len);

        // Compile-time assertion: Entity::PLACEHOLDER has all bits set to 1.
        const {
            assert!(Entity::PLACEHOLDER.to_bits() == u64::MAX);
        }

        unsafe {
            // Efficiently initialize all slots with the placeholder value.
            // Equivalent to memset with 0xFF.
            // `count` must be `len` instead of `len * size_of<Entity>`,
            // see in `write_bytes` docs
            boxed.as_mut_ptr().write_bytes(u8::MAX, len);
        }

        let ptr = Box::leak(boxed).as_mut_ptr() as *mut Entity;
        self.head.store(ptr, Ordering::Relaxed);
        ptr
    }

    /// Deallocates memory for the chunk.
    ///
    /// # Safety
    /// - The caller must ensure concurrency safety.
    /// - `capacity` must match the value used during allocation.
    /// - The chunk must have been previously allocated.
    unsafe fn dealloc(&mut self, capacity: u32) {
        let data = *self.head.get_mut();
        if !data.is_null() {
            let len = capacity as usize;
            let slice = ptr::slice_from_raw_parts_mut(data, len);
            unsafe {
                ::core::mem::drop(Box::from_raw(slice));
            }
        }
    }

    /// Retrieves an entity at the specified index within this chunk.
    ///
    /// # Safety
    /// - The caller must ensure concurrency safety.
    /// - The index must be within the chunk's capacity.
    /// - The chunk must be initialized (head is not null).
    #[inline]
    unsafe fn get(&self, index: u32) -> Entity {
        let head = self.head.load(Ordering::Relaxed);
        unsafe { *head.add(index as usize) }
    }

    /// Retrieves a slice of entities starting at the specified index.
    ///
    /// # Safety
    /// - The caller must ensure concurrency safety.
    /// - The index and required length must be within bounds.
    /// - The chunk must be initialized (head is not null).
    #[inline]
    unsafe fn get_slice(&self, index: u32, required_len: u32, chunk_capacity: u32) -> &[Entity] {
        let available_len = chunk_capacity - index;
        let len = available_len.min(required_len) as usize;

        let head = self.head.load(Ordering::Relaxed);
        unsafe { slice::from_raw_parts(head.add(index as usize), len) }
    }

    /// Stores a slice of entities starting at the specified index.
    ///
    /// # Safety
    /// - The caller must ensure concurrency safety.
    /// - The index must be within the chunk's capacity.
    /// - The chunk will be allocated if not already initialized.
    ///
    /// # Returns
    /// The number of entities successfully stored.
    #[inline]
    unsafe fn set_slice(&self, index: u32, entities: &[Entity], chunk_capacity: u32) -> usize {
        let available_len = (chunk_capacity - index) as usize;
        let len = available_len.min(entities.len());

        let mut head = self.head.load(Ordering::Relaxed);
        if head.is_null() {
            unsafe {
                head = self.alloc(chunk_capacity);
            }
        }

        unsafe {
            let target = head.add(index as usize);
            ptr::copy_nonoverlapping(entities.as_ptr(), target, len);
        }

        len
    }
}

// -----------------------------------------------------------------------------
// FreeBuffer

const NUM_CHUNKS: u32 = 24;
const NUM_SKIPPED: u32 = u32::BITS - NUM_CHUNKS;

/// A buffer composed of chunks with exponentially increasing capacities.
/// Chunk capacities follow the pattern: `[512, 512, 1024, 2048, 4096, ...]`
struct FreeBuffer([Chunk; NUM_CHUNKS as usize]);

impl FreeBuffer {
    /// Constructs an empty [`FreeBuffer`] with all chunks uninitialized.
    const fn new() -> Self {
        Self([const { Chunk::new() }; NUM_CHUNKS as usize])
    }

    /// Returns the capacity of the chunk at the specified index.
    #[inline]
    const fn chunck_capacity(chunk_index: u32) -> u32 {
        // Capacities: 512, 512, 1024, 2048, 4096, ...
        let corrected = if chunk_index == 0 { 1 } else { chunk_index };
        let corrected = corrected + NUM_SKIPPED;
        1 << corrected
    }

    /// Locates the chunk containing the specified global index.
    ///
    /// Returns a tuple containing:
    /// - Reference to the chunk
    /// - Index within that chunk
    /// - Capacity of that chunk
    #[inline]
    fn chunk_with_index(&self, full_index: u32) -> (&Chunk, u32, u32) {
        // Optimization: Determine chunk index based on the position of the highest set bit.
        // For example, the chunk index of `0b1000...000` is `23` (the last chunk).
        let chunk_index = (NUM_CHUNKS - 1).saturating_sub(full_index.leading_zeros());

        let chunk = unsafe { self.0.get_unchecked(chunk_index as usize) };
        let chunk_capacity = Self::chunck_capacity(chunk_index);
        // ↓ Eq to `full_index & (chunk_capacity - 1)`, but faster.
        let index_in_chunk = full_index & !chunk_capacity;
        (chunk, index_in_chunk, chunk_capacity)
    }

    /// Retrieves an entity at the specified global index.
    ///
    /// # Safety
    /// - The caller must ensure concurrency safety.
    /// - The index must be within the buffer's current logical length.
    #[inline]
    unsafe fn get(&self, full_index: u32) -> Entity {
        let (chunk, index, _) = self.chunk_with_index(full_index);
        // SAFETY: Ensured by caller.
        unsafe { chunk.get(index) }
    }

    /// Stores a slice of entities starting at the specified global index.
    ///
    /// # Safety
    /// - The caller must ensure concurrency safety.
    /// - The operation may span multiple chunks if the slice crosses chunk boundaries.
    unsafe fn set_slice(&self, mut full_index: u32, mut entities: &[Entity]) {
        while !entities.is_empty() {
            let (chunk, index, chunk_capacity) = self.chunk_with_index(full_index);

            unsafe {
                let len = chunk.set_slice(index, entities, chunk_capacity);
                full_index += len as u32;
                entities = &entities[len..];
            }
        }
    }

    /// Creates an iterator over a range of indices in the buffer.
    ///
    /// # Safety
    /// - The caller must ensure concurrency safety.
    /// - All indices in the range must have been previously initialized via [`Self::set_slice`].
    #[inline]
    unsafe fn iter(&self, indices: core::ops::Range<u32>) -> FreeBufferIter<'_> {
        FreeBufferIter {
            buffer: self,
            current_iter: [].iter(),
            remaining_indices: indices,
        }
    }
}

impl Drop for FreeBuffer {
    fn drop(&mut self) {
        for index in 0..NUM_CHUNKS {
            let capacity = Self::chunck_capacity(index);
            // SAFETY: Exclusive access (&mut self) and correct capacity.
            unsafe { self.0[index as usize].dealloc(capacity) };
        }
    }
}

/// Iterator over entities in a [`FreeBuffer`].
struct FreeBufferIter<'a> {
    buffer: &'a FreeBuffer,
    current_iter: slice::Iter<'a, Entity>,
    remaining_indices: core::ops::Range<u32>,
}

impl Iterator for FreeBufferIter<'_> {
    type Item = Entity;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        #[cold]
        #[inline(never)]
        fn slow_next(this: &mut FreeBufferIter<'_>, required: u32) -> Option<Entity> {
            let next_index = this.remaining_indices.start;
            let (chunk, index, capacity) = this.buffer.chunk_with_index(next_index);

            let slice = unsafe { chunk.get_slice(index, required, capacity) };
            this.remaining_indices.start = next_index + slice.len() as u32;
            this.current_iter = slice.iter();

            let next = unsafe { this.current_iter.next().debug_checked_unwrap() };
            Some(*next)
        }

        // First, try to get an entity from the current chunk slice
        if let Some(&entity) = self.current_iter.next() {
            return Some(entity);
        }

        // If current slice is exhausted, fetch the next chunk
        let still_need = self.remaining_indices.len() as u32;
        if still_need == 0 {
            None
        } else {
            slow_next(self, still_need)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.current_iter.len() + self.remaining_indices.len();
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for FreeBufferIter<'a> {}
impl<'a> FusedIterator for FreeBufferIter<'a> {}

// -----------------------------------------------------------------------------
// FreeCount

/// Packed state representation for [`FreeCount`].
/// Encodes length, disable flag, and generation counter in a single u64.
#[derive(Clone, Copy)]
#[repr(transparent)]
struct FreeCountState(u64);

impl FreeCountState {
    /// Bit position for the disable flag.
    /// When set, remote allocations are blocked.
    const DISABLING_BIT: u64 = 1 << 33;

    /// Bitmask for the length field (33 bits).
    const LENGTH_MASK: u64 = (1 << 32) | (u32::MAX as u64);

    /// Encoded value representing length = 0.
    const LENGTH_0: u64 = 1 << 32;

    /// Least significant bit of the 30-bit generation counter.
    const GENERATION_LEAST_BIT: u64 = 1 << 34;

    /// Creates a state with zero length, first generation, and not disabled.
    const fn zero() -> Self {
        Self(Self::LENGTH_0)
    }

    /// Extracts the logical length from the packed state.
    #[inline]
    const fn length(self) -> u32 {
        let unsigned_length = self.0 & Self::LENGTH_MASK;
        unsigned_length.saturating_sub(Self::LENGTH_0) as u32
    }

    /// Checks if the disable flag is set.
    #[inline]
    const fn is_disabled(self) -> bool {
        (self.0 & Self::DISABLING_BIT) > 0
    }

    /// Creates a new state with only the length changed.
    #[inline]
    const fn with_length(self, length: u32) -> Self {
        // Encode length with the "zero offset" bit set
        let length = length as u64 | Self::LENGTH_0;
        Self(self.0 & !Self::LENGTH_MASK | length)
    }

    /// Encodes a "pop" operation for the given number of elements.
    /// This simultaneously subtracts from length and increments the generation.
    #[inline]
    const fn encode_generation(num: u32) -> u64 {
        let subtract_length = num as u64;
        // Subtract from length AND increment generation (by subtracting from generation bits)
        subtract_length | Self::GENERATION_LEAST_BIT
    }

    /// Applies a pop operation to the state.
    #[inline]
    const fn pop(self, num: u32) -> Self {
        Self(self.0.wrapping_sub(Self::encode_generation(num)))
    }
}

/// Atomic interface for [`FreeCountState`].
struct FreeCount(AtomicU64);

impl FreeCount {
    /// Creates a new [`FreeCount`] initialized to zero.
    const fn new() -> Self {
        Self(AtomicU64::new(FreeCountState::zero().0))
    }

    /// Loads the current state with the specified memory ordering.
    #[inline]
    fn acquire_state(&self) -> FreeCountState {
        FreeCountState(self.0.load(Ordering::Acquire))
    }

    /// Atomically subtracts `num` from the length, returning the previous state.
    ///
    /// # Note
    /// The caller must ensure that:
    /// - Changing the state is permitted (not disabled)
    /// - Sufficient elements exist to pop
    #[inline]
    fn pop_for_state(&self, num: u32) -> FreeCountState {
        let to_sub = FreeCountState::encode_generation(num);
        let raw = self.0.fetch_sub(to_sub, Ordering::Acquire);
        FreeCountState(raw)
    }

    /// Sets the disable flag, returning the previous state.
    #[inline]
    fn disable_for_state(&self) -> FreeCountState {
        // Generation change is irrelevant here since we're modifying the value anyway
        FreeCountState(self.0.fetch_or(FreeCountState::DISABLING_BIT, Ordering::Acquire))
    }

    /// Stores a new state value.
    ///
    /// # Safety
    /// This is "risky" because it doesn't verify that the state hasn't changed
    /// since it was read. Incorrect use may cause entities to be skipped or
    /// allocated multiple times.
    #[inline]
    fn set_state_risky(&self, state: FreeCountState) {
        self.0.store(state.0, Ordering::Release);
    }

    /// Attempts to update the state atomically using compare-and-swap.
    #[inline]
    fn try_set_state(
        &self,
        expected_current_state: FreeCountState,
        target_state: FreeCountState,
    ) -> Result<(), FreeCountState> {
        match self.0.compare_exchange(
            expected_current_state.0,
            target_state.0,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(val) => Err(FreeCountState(val)),
        }
    }
}

// -----------------------------------------------------------------------------
// FreeList

/// Thread-safe collection of recycled entities, similar to `Vec<Entity>` but optimized
/// for concurrent access and remote allocation scenarios.
struct FreeList {
    /// Storage buffer for entities.
    buffer: FreeBuffer,
    /// Atomic state tracking length, disable flag, and generation.
    len: FreeCount,
}

impl FreeList {
    /// Creates an empty [`FreeList`].
    #[inline]
    const fn new() -> Self {
        Self {
            buffer: FreeBuffer::new(),
            len: FreeCount::new(),
        }
    }

    /// Returns the current number of free entities.
    #[inline]
    fn count(&self) -> u32 {
        // Relaxed ordering suffices for read-only observation
        self.len.acquire_state().length()
    }

    /// Adds entities to the free list for reuse.
    ///
    /// # Safety
    /// - The caller must ensure exclusive access or proper synchronization.
    unsafe fn free(&self, entities: &[Entity]) {
        // Block remote allocations during this operation
        let state = self.len.disable_for_state();

        // Append entities to the buffer
        let full_index = state.length();
        unsafe {
            self.buffer.set_slice(full_index, entities);
        }

        // Update length and re-enable allocations
        let new_state = state.with_length(full_index + entities.len() as u32);
        self.len.set_state_risky(new_state);
    }

    /// Allocates a single entity from the free list.
    ///
    /// # Safety
    /// - The caller must ensure exclusive access or proper synchronization.
    #[inline]
    unsafe fn alloc(&self) -> Option<Entity> {
        let len = self.len.pop_for_state(1).length();
        let index = len.checked_sub(1)?;

        Some(unsafe { self.buffer.get(index) })
    }

    /// Allocates multiple entities from the free list.
    ///
    /// # Safety
    /// - The caller must ensure exclusive access or proper synchronization.
    #[inline]
    unsafe fn alloc_many(&self, count: u32) -> FreeBufferIter<'_> {
        let len = self.len.pop_for_state(count).length();
        let index = len.saturating_sub(count);

        unsafe { self.buffer.iter(index..len) }
    }

    /// Allocates an entity safely from a remote context.
    /// Uses compare-and-swap loops to handle concurrent modifications.
    #[inline]
    fn remote_alloc(&self) -> Option<Entity> {
        let backoff = Backoff::new();
        let mut state = self.len.acquire_state();

        loop {
            // Wait if free operations are in progress
            if state.is_disabled() {
                backoff.snooze();
                state = self.len.acquire_state();
                continue;
            }

            let len = state.length();
            let index = len.checked_sub(1)?;

            // Read the entity before attempting to claim it
            let entity = unsafe { self.buffer.get(index) };
            let new_state = state.pop(1);

            // Attempt to atomically claim this entity
            match self.len.try_set_state(state, new_state) {
                Ok(_) => return Some(entity),
                Err(actual) => state = actual, // Retry with updated state
            }
        }
    }
}

// -----------------------------------------------------------------------------
// FreshAllocator

/// # Safety
/// `id != 0 && id != u32::MAX`
#[inline(always)]
unsafe fn entity_from_u32(id: u32) -> Entity {
    let id = unsafe { core::mem::transmute::<u32, EntityId>(id) };
    Entity::new(id, EntityTag::FIRST)
}

/// Allocator for new, never-before-used entity IDs.
struct FreshAllocator {
    next_id: AtomicU32,
}

impl FreshAllocator {
    /// Maximum number of entities that can be allocated.
    /// The valid `EntityId` range is `1..u32::MAX`
    const MAX_ENTITIES: u32 = u32::MAX;

    /// Panic handler for overflow conditions.
    #[cold]
    #[inline(never)]
    fn on_overflow() -> ! {
        panic!("too many entities")
    }

    /// Creates a new [`FreshAllocator`] starting from ID 1.
    #[inline]
    const fn new() -> FreshAllocator {
        // Start from 1 (0 is reserved for invalid/null entities)
        FreshAllocator {
            next_id: AtomicU32::new(1),
        }
    }

    /// Returns the total number of entity IDs allocated so far.
    #[inline]
    fn count(&self) -> u32 {
        self.next_id.load(Ordering::Relaxed) - 1
    }

    /// Allocates a single new entity ID.
    #[inline]
    fn alloc(&self) -> Entity {
        let index = self.next_id.fetch_add(1, Ordering::Relaxed);
        if index == Self::MAX_ENTITIES {
            Self::on_overflow();
        }
        // SAFETY: `next_id` starts from 1 and increments, never reaching 0
        unsafe { entity_from_u32(index) }
    }

    /// Allocates multiple new entity IDs.
    fn alloc_many(&self, count: u32) -> FreshEntityIter {
        if count == 0 {
            return FreshEntityIter(0..0);
        }

        let start = self.next_id.fetch_add(count, Ordering::Relaxed);
        // Check for overflow before it happens
        if start > Self::MAX_ENTITIES - count {
            Self::on_overflow();
        }

        FreshEntityIter(start..(start + count))
    }
}

/// Iterator over freshly allocated entity IDs.
struct FreshEntityIter(core::ops::Range<u32>);

impl Iterator for FreshEntityIter {
    type Item = Entity;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|index| unsafe { entity_from_u32(index) })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl ExactSizeIterator for FreshEntityIter {}
impl FusedIterator for FreshEntityIter {}

// -----------------------------------------------------------------------------
// AllocEntitiesIter

/// Iterator that yields entities from both recycled and fresh sources.
pub struct AllocEntitiesIter<'a> {
    fresh: FreshEntityIter,
    reused: FreeBufferIter<'a>,
}

impl Iterator for AllocEntitiesIter<'_> {
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        // Prioritize recycled entities before allocating new ones
        self.reused.next().or_else(|| self.fresh.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.reused.len() + self.fresh.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for AllocEntitiesIter<'_> {}
impl FusedIterator for AllocEntitiesIter<'_> {}

impl Drop for AllocEntitiesIter<'_> {
    fn drop(&mut self) {
        let leaking = self.len();
        if leaking > 0 {
            log::warn!("{leaking} entities being leaked via unfinished `AllocEntitiesIter`");
        }
    }
}

// -----------------------------------------------------------------------------
// SharedAllocator

/// Shared state between [`EntityAllocator`] and [`RemoteAllocator`].
/// Provides thread-safe entity allocation with support for both
/// in-world and remote allocation scenarios.
struct SharedAllocator {
    /// Recycled entities available for reuse
    free: FreeList,
    /// Allocator for new entity IDs
    fresh: FreshAllocator,
    /// Flag indicating whether the allocator has been closed
    is_closed: AtomicBool,
}

// -----------------------------------------------------------------------------
// RemoteAllocator

/// Entity allocator that can operate without direct `World` access.
///
/// Useful for asynchronous operations, background tasks, or any scenario
/// where entity allocation is needed but holding a `World` reference is
/// impractical or impossible.
///
/// # Safety Considerations
/// - Entities allocated remotely may become invalid if the source `World`
///   is destroyed before they are used.
/// - Always verify allocation validity using [`RemoteAllocator::is_closed`]
///   or [`EntityAllocator::is_connected_to`] before using remotely allocated entities.
#[derive(Clone)]
pub struct RemoteAllocator {
    shared: Arc<SharedAllocator>,
}

impl RemoteAllocator {
    /// Checks whether the allocator has been closed.
    ///
    /// The allocator is closed when the parent [`EntityAllocator`] is dropped,
    /// which typically indicates that the `World` has been destroyed.
    ///
    /// # Returns
    /// `true` if the allocator is closed and allocations should not be used.
    #[inline]
    pub fn is_closed(&self) -> bool {
        self.shared.is_closed.load(Ordering::Acquire)
    }

    /// Determines if this [`RemoteAllocator`] is connected to the same
    /// `World` as the provided [`EntityAllocator`].
    #[inline]
    pub fn is_connected_to(&self, source: &EntityAllocator) -> bool {
        Arc::ptr_eq(&self.shared, &source.shared)
    }

    /// Allocates a single entity.
    ///
    /// Attempts to reuse a recycled entity first, falling back to
    /// allocating a new entity if none are available.
    pub fn alloc(&self) -> Entity {
        self.shared
            .free
            .remote_alloc()
            .unwrap_or_else(|| self.shared.fresh.alloc())
    }
}

impl Debug for RemoteAllocator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RemoteAllocator")
            .field("allocated", &self.shared.fresh.count())
            .field("recycled", &self.shared.free.count())
            .finish()
    }
}

// -----------------------------------------------------------------------------
// EntityAllocator

/// Local buffer size for batching free operations.
/// This amortizes the cost of synchronization with the shared allocator.
const LOCAL_CAP: usize = 127;

/// Local buffer whose contents are preferred when
/// a mutable reference is available.
///
/// Note: the free buffer and the allocation buffer
/// are kept separate rather than combined. This helps
/// avoid hot entities being rapidly reallocated,
/// which can cause generation counters to advance
/// quickly and increase the risk of id reuse/collision.
struct LocalBuffer {
    free: ArrayVec<Entity, LOCAL_CAP>,
    alloc: ArrayVec<Entity, LOCAL_CAP>,
}

/// Primary entity allocator bound to a `World` instance.
///
/// Manages both allocation of new entities and recycling of destroyed entities.
/// This is an internal type; entity allocation is automatically handled by
/// `World` when creating entities.
///
/// # Important Notes
/// - Entities are specific to their creating `World` and cannot be used
///   across different `World` instances.
/// - The allocator does not modify entity `Generation` values during
///   allocation or recycling. Callers must manage generation increments
///   separately to prevent aliasing of recycled entity IDs.
pub struct EntityAllocator {
    shared: Arc<SharedAllocator>,
    local: Box<LocalBuffer>,
}

impl Debug for EntityAllocator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EntityAllocator")
            .field("allocated", &self.shared.fresh.count())
            .field("recycled", &self.shared.free.count())
            .finish()
    }
}

impl Drop for EntityAllocator {
    fn drop(&mut self) {
        // Signal to remote allocators that this allocator is no longer valid
        self.shared.is_closed.store(true, Ordering::Release);
    }
}

impl EntityAllocator {
    /// Creates a new [`EntityAllocator`].
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::new(SharedAllocator {
                free: FreeList::new(),
                fresh: FreshAllocator::new(),
                is_closed: AtomicBool::new(false),
            }),
            local: Box::new(LocalBuffer {
                free: ArrayVec::new(),
                alloc: ArrayVec::new(),
            }),
        }
    }

    /// Creates a [`RemoteAllocator`] from this allocator.
    ///
    /// The remote allocator can be used to allocate entities without
    /// requiring direct access to the `World`.
    #[inline]
    pub fn build_remote(&self) -> RemoteAllocator {
        RemoteAllocator {
            shared: self.shared.clone(),
        }
    }

    /// Checks if a [`RemoteAllocator`] is connected to this allocator.
    #[inline]
    pub fn is_connected_to(&self, remote: &RemoteAllocator) -> bool {
        Arc::ptr_eq(&self.shared, &remote.shared)
    }

    /// Recycles a single entity for future reuse.
    ///
    /// Note: Entities may be stored in a local buffer and not immediately
    /// made available for allocation until the buffer is flushed.
    #[inline]
    pub fn free(&mut self, entity: Entity) {
        #[cold]
        #[inline(never)]
        fn flush_freed(this: &mut EntityAllocator) {
            // SAFETY: We have exclusive access (&mut self)
            unsafe {
                let local_free = &mut this.local.free;
                this.shared.free.free(local_free.as_slice());
                local_free.set_len(0);
            }
        }

        // Flush local buffer if full
        if self.local.free.is_full() {
            flush_freed(self);
        }

        // Add entity to local buffer
        unsafe {
            self.local.free.push_unchecked(entity);
        }
    }

    /// Recycles multiple entities for future reuse.
    ///
    /// More efficient than individual [`free`](Self::free) calls for batches.
    pub fn free_many(&mut self, entities: &[Entity]) {
        let local_free = &mut self.local.free;
        if LOCAL_CAP - local_free.len() >= entities.len() {
            let old_len = local_free.len();
            let append = entities.len();
            let new_len = old_len + append;
            unsafe {
                ptr::copy_nonoverlapping::<Entity>(
                    entities.as_ptr(),
                    local_free.as_mut_ptr().add(old_len),
                    append,
                );
                local_free.set_len(new_len);
            }
        } else {
            unsafe {
                self.shared.free.free(entities);
            }
        }
    }

    /// Allocates a single entity with mutable access, checking local buffer first.
    ///
    /// More efficient than [`alloc`](Self::alloc) when mutable access is available.
    #[must_use]
    pub fn alloc_mut(&mut self) -> Entity {
        #[cold]
        #[inline(never)]
        fn alloc_slow(this: &mut EntityAllocator) -> Entity {
            let local_alloc = &mut this.local.alloc;

            let count = LOCAL_CAP as u32 + 1;
            let mut reused = unsafe { this.shared.free.alloc_many(count) };
            let still_need = count - reused.len() as u32;
            let mut fresh = this.shared.fresh.alloc_many(still_need);

            let mut ret = reused.next();
            reused.for_each(|v| unsafe {
                local_alloc.push_unchecked(v);
            });
            if ret.is_none() {
                ret = fresh.next();
            }
            fresh.for_each(|v| unsafe {
                local_alloc.push_unchecked(v);
            });

            debug_assert!(local_alloc.len() == LOCAL_CAP);

            unsafe { ret.debug_checked_unwrap() }
        }

        if let Some(entity) = self.local.alloc.pop() {
            entity
        } else {
            alloc_slow(self)
        }
    }

    /// Allocates a single entity, preferring recycled entities.
    ///
    /// Note: Does not modify the entity's `Generation`. Callers must
    /// increment generation when reusing entity IDs to prevent aliasing.
    #[must_use]
    pub fn alloc(&self) -> Entity {
        unsafe { self.shared.free.alloc() }.unwrap_or_else(|| self.shared.fresh.alloc())
    }

    /// Efficiently allocates multiple entities.
    ///
    /// Returns an iterator that must be fully consumed; otherwise,
    /// any remaining entities will be leaked (not available for reuse).
    #[must_use]
    pub fn alloc_many(&self, count: u32) -> AllocEntitiesIter<'_> {
        // SAFETY: Caller ensures exclusive access or proper synchronization
        let reused = unsafe { self.shared.free.alloc_many(count) };
        let still_need = count - reused.len() as u32;
        let fresh = self.shared.fresh.alloc_many(still_need);
        AllocEntitiesIter { fresh, reused }
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::{EntityAllocator, FreeBuffer};
    use alloc::vec::Vec;

    #[test]
    fn chunck_capacity() {
        assert!(FreeBuffer::chunck_capacity(0) == 512);
        assert!(FreeBuffer::chunck_capacity(1) == 512);
        assert!(FreeBuffer::chunck_capacity(2) == 1024);
        assert!(FreeBuffer::chunck_capacity(3) == 2048);
    }

    #[test]
    fn uniqueness() {
        let mut entities = Vec::with_capacity(2000);
        let mut allocator = EntityAllocator::new();

        entities.extend(allocator.alloc_many(1000));

        let pre_len = entities.len();
        entities.sort();
        entities.dedup();
        assert_eq!(pre_len, entities.len(), "fail 1");

        entities.drain(500..).for_each(|e| allocator.free(e));
        allocator.free_many(&entities);
        entities.clear();

        entities.extend(allocator.alloc_many(500));
        (0..500).for_each(|_| entities.push(allocator.alloc()));
        (0..500).for_each(|_| entities.push(allocator.alloc_mut()));
        entities.extend(allocator.alloc_many(500));

        let pre_len = entities.len();
        entities.sort();
        entities.dedup();
        assert_eq!(pre_len, entities.len(), "fail 2");
    }

    #[test]
    fn recyclable() {
        let mut entities = Vec::with_capacity(1000);
        let mut allocator = EntityAllocator::new();

        for _ in 0..50 {
            (0..150).for_each(|_| entities.push(allocator.alloc()));
            (0..150).for_each(|_| entities.push(allocator.alloc_mut()));
            entities.extend(allocator.alloc_many(200));

            // We only allocated 500 units, but there is a buffer inside the allocator.
            // So the maximum entity index will exceed 500, but it shouldn't be much bigger.
            assert!(entities.iter().all(|t| t.index() < 1500));

            entities.drain(300..).for_each(|e| allocator.free(e));
            allocator.free_many(&entities);
            entities.clear();
        }
    }
}
