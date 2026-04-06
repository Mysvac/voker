use core::fmt::{Debug, Display};
use core::hash::Hash;

use voker_utils::num::NonMaxU32;

// -----------------------------------------------------------------------------
// ArcheId

/// Unique identifier for an [`Archetype`].
///
/// [`Archetype`]: super::Archetype
#[derive(Copy, Clone, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ArcheId(NonMaxU32);

impl ArcheId {
    /// Id of the empty archetype (no components).
    pub const EMPTY: ArcheId = ArcheId(NonMaxU32::ZERO);

    /// # Panics
    /// Panic if id == u32::MAX.
    #[inline(always)]
    pub(crate) const fn new(id: u32) -> Self {
        Self(NonMaxU32::new(id).expect("too many archetypes"))
    }

    /// # Safety
    /// The value must not be the maximum value of the underlying integer type.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(id: u32) -> Self {
        Self(unsafe { NonMaxU32::new_unchecked(id) })
    }

    /// Creates a new `ArcheId` from a usize.
    ///
    /// # Panics
    /// Panic if `id` >= u32::MAX.
    #[inline(always)]
    pub const fn without_provenance(id: usize) -> Self {
        if id >= u32::MAX as usize {
            voker_utils::cold_path();
            panic!("ArcheId must be < u32::MAX");
        }
        unsafe { Self(NonMaxU32::new_unchecked(id as u32)) }
    }

    /// Returns the archetype index as a usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }
}

impl Debug for ArcheId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0.get(), f)
    }
}

impl Display for ArcheId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0.get(), f)
    }
}

impl Hash for ArcheId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Sparse hashing is optimized for smaller values.
        // So we use represented values, rather than the underlying bits.
        state.write_u32(self.0.get());
    }
}

impl PartialEq for ArcheId {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for ArcheId {}

// -----------------------------------------------------------------------------
// ArcheRow

/// Represents a row of an entity within an [`Archetype`].
///
/// The `Archetype` maintains a contiguous array of entities.
/// `ArcheRow` is the index of an entity within that array.
///
/// [`Archetype`]: super::Archetype
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct ArcheRow(pub u32);

impl Debug for ArcheRow {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for ArcheRow {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Hash for ArcheRow {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.0);
    }
}
