use core::fmt::{Debug, Display};
use core::hash::Hash;

use voker_utils::num::NonMaxU32;

// -----------------------------------------------------------------------------
// BundleId

/// Unique identifier for a Bundle.
#[derive(Copy, Clone, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BundleId(NonMaxU32);

impl BundleId {
    pub const EMPTY: BundleId = BundleId(NonMaxU32::ZERO);

    #[inline(always)]
    pub(crate) const fn new(id: u32) -> Self {
        Self(NonMaxU32::new(id).expect("too many bundles"))
    }

    /// Returns the bundle index as a usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }
}

impl Debug for BundleId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0.get(), f)
    }
}

impl Display for BundleId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0.get(), f)
    }
}

impl Hash for BundleId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Sparse hashing is optimized for smaller values.
        // So we use represented values, rather than the underlying bits
        state.write_u32(self.0.get());
    }
}

impl PartialEq for BundleId {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for BundleId {}
