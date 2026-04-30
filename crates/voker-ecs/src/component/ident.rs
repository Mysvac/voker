use core::fmt::{Debug, Display};
use core::hash::Hash;

use voker_utils::num::NonMaxU32;

// -----------------------------------------------------------------------------
// ComponentId

/// A unique identifier for a `Component` type within a specific `World`.
///
/// `ComponentId` provides a type-safe way to identify component types at
/// runtime. These IDs are only valid within the context of a single `World`
/// instance and are not globally unique across different worlds.
///
/// The ID is stored as a `NonMaxU32` to enable memory layout optimizations,
/// allowing `Option<ComponentId>` to be the same size as `ComponentId` itself.
#[derive(Clone, Copy, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ComponentId(NonMaxU32);

impl ComponentId {
    #[inline]
    pub(crate) const fn new(id: u32) -> Self {
        Self(NonMaxU32::new(id).expect("too many components"))
    }

    /// Creates a new `ComponentId` from a usize.
    ///
    /// # Panics
    /// Panics if `id >= u32::MAX`.
    #[inline(always)]
    pub const fn without_provenance(id: usize) -> Self {
        if id >= u32::MAX as usize {
            core::hint::cold_path();
            panic!("ComponentId must be < u32::MAX");
        }
        unsafe { Self(NonMaxU32::new_unchecked(id as u32)) }
    }

    /// Convert `ComponentId` to usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }
}

impl PartialEq for ComponentId {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for ComponentId {}

impl Hash for ComponentId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Sparse hashing is optimized for smaller values.
        // So we use represented values, rather than the underlying bits
        state.write_u32(self.0.get());
    }
}

impl Debug for ComponentId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0.get(), f)
    }
}

impl Display for ComponentId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0.get(), f)
    }
}
