use core::fmt::{Debug, Display};
use core::hash::Hash;

use voker_utils::num::NonMaxU32;

// -----------------------------------------------------------------------------
// ResourceId

/// A unique identifier for a `Resource` type within a specific `World`.
///
/// `ResourceId` provides a type-safe way to identify resource types at
/// runtime. These IDs are only valid within the context of a single `World`
/// instance and are not globally unique across different worlds.
///
/// The ID is stored as a `NonMaxU32` to enable memory layout optimizations,
/// allowing `Option<ResourceId>` to be the same size as `ResourceId` itself.
#[derive(Clone, Copy, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ResourceId(NonMaxU32);

impl ResourceId {
    #[inline]
    pub(crate) const fn new(id: u32) -> Self {
        Self(NonMaxU32::new(id).expect("too many resources"))
    }

    /// Convert `ResourceId` to usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }
}

impl PartialEq for ResourceId {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for ResourceId {}

impl Hash for ResourceId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Sparse hashing is optimized for smaller values.
        // So we use represented values, rather than the underlying bits
        state.write_u32(self.0.get());
    }
}

impl Debug for ResourceId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0.get(), f)
    }
}

impl Display for ResourceId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0.get(), f)
    }
}
