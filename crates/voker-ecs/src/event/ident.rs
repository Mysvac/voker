use core::fmt::{Debug, Display};
use core::hash::Hash;

use voker_utils::num::NonMaxU32;

// -----------------------------------------------------------------------------
// EventId

/// A unique identifier for a `Event` type within a specific `World`.
#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
#[repr(transparent)]
pub struct EventId(NonMaxU32);

impl EventId {
    #[inline]
    pub(crate) const fn new(id: u32) -> Self {
        Self(NonMaxU32::new(id).expect("too many events"))
    }

    /// Creates a new `EventId` from a usize.
    ///
    /// # Panics
    /// Panics if `id >= u32::MAX`.
    #[inline(always)]
    pub const fn without_provenance(id: usize) -> Self {
        if id >= u32::MAX as usize {
            voker_utils::cold_path();
            panic!("EventId must be < u32::MAX");
        }
        unsafe { Self(NonMaxU32::new_unchecked(id as u32)) }
    }

    /// Convert `EventId` to usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }
}

impl Hash for EventId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Sparse hashing is optimized for smaller values.
        // So we use represented values, rather than the underlying bits
        state.write_u32(self.0.get());
    }
}

impl Debug for EventId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0.get(), f)
    }
}

impl Display for EventId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0.get(), f)
    }
}
