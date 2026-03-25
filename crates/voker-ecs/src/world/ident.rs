use core::fmt::{Debug, Display};
use core::hash::Hash;
use core::num::NonZeroUsize;
use voker_os::sync::atomic::{AtomicUsize, Ordering::Relaxed};

// -----------------------------------------------------------------------------
// WorldId

/// A unique identifier for a World instance in the ECS.
///
/// Use [`WorldId::alloc`] to allocate a new ID, which guarantees global uniqueness.
///
/// IDs are allocated sequentially starting from 1 and increment by 1 each time.
/// Allocation will panic if the value exceeds `usize::MAX`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorldId(NonZeroUsize);

impl WorldId {
    /// Creates a new `WorldId` with the given raw value.
    pub fn alloc() -> Self {
        static ALLOCATOR: AtomicUsize = AtomicUsize::new(1);

        let next = ALLOCATOR
            .fetch_update(Relaxed, Relaxed, |val| val.checked_add(1))
            .expect("too many worlds");

        // `1..usize::MAX`
        WorldId(NonZeroUsize::new(next).unwrap())
    }

    /// Returns the raw index value of this id as a `usize`.
    #[inline]
    pub const fn index(self) -> usize {
        self.0.get()
    }
}

impl Hash for WorldId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.0.get());
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
