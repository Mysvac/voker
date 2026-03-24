use core::fmt::{Debug, Display};
use core::hash::Hash;
use core::num::NonZeroU64;
use voker_os::sync::atomic::{AtomicU64, Ordering};

// -----------------------------------------------------------------------------
// WorldId

/// A unique identifier for a World instance in the ECS.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorldId(NonZeroU64);

impl WorldId {
    /// Creates a new `WorldId` with the given raw value.
    #[inline]
    pub const fn new(id: NonZeroU64) -> Self {
        Self(id)
    }

    /// Returns the raw index value of this id as a `usize`.
    #[inline]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }

    /// Returns the raw index value of this id as a `u64`.
    #[inline]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl Hash for WorldId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.0.get());
    }
}

impl Debug for WorldId {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for WorldId {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl From<WorldId> for u64 {
    #[inline]
    fn from(value: WorldId) -> Self {
        value.0.get()
    }
}

impl From<WorldId> for NonZeroU64 {
    #[inline]
    fn from(value: WorldId) -> Self {
        value.0
    }
}

// -----------------------------------------------------------------------------
// WorldIdAllocator

/// A thread-safe allocator for generating unique [`WorldId`]s.
///
/// If you create an object using [`World::default`],
/// this allocator will be used implicitly.
///
/// # Examples
///
/// ```
/// # use voker_ecs::world::WorldIdAllocator;
/// static ALLOCATOR: WorldIdAllocator = WorldIdAllocator::new();
///
/// fn spawn_world() {
///     let world_id = ALLOCATOR.alloc();
///     // Use the unique world ID...
/// }
/// ```
///
/// # Panics
///
/// The allocator will panic if more than `u64::MAX` worlds are
/// created in a single program execution.
///
/// [`World::default`]: crate::world::World::default
#[derive(Debug, Default)]
pub struct WorldIdAllocator {
    next: AtomicU64,
}

impl WorldIdAllocator {
    /// Creates a new `WorldIdAllocator` starting from ID `1`.
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(1),
        }
    }

    /// Returns how many ids have been handed out so far.
    pub fn count(&self) -> usize {
        self.next.load(Ordering::Relaxed) as usize - 1
    }

    /// Allocates a new unique [`WorldId`].
    ///
    /// # Panics
    ///
    /// Panics if the internal counter overflows (i.e., more than `u64::MAX` worlds
    /// have been allocated). This is extremely unlikely in practice.
    pub fn alloc(&self) -> WorldId {
        let next = self.next.fetch_add(1, Ordering::Relaxed);
        assert!(next < u64::MAX, "too many worlds");
        // SAFETY: `next` start from `1`.
        WorldId(NonZeroU64::new(next).unwrap())
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::WorldIdAllocator;

    #[test]
    fn alloc() {
        let allocator = WorldIdAllocator::new();
        assert_eq!(allocator.alloc().index(), 1);
        assert_eq!(allocator.alloc().index(), 2);
        assert_eq!(allocator.alloc().index(), 3);
    }
}
