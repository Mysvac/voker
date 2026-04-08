use core::fmt::{Debug, Display};
use core::hash::Hash;

use voker_utils::num::NonMaxU32;

// -----------------------------------------------------------------------------
// MapId

/// Unique identifier for a Map in the ECS storage.
///
/// `MapId` is an index for Map, and also represents a sparse Component.
///
/// Wraps a non-max u16 to provide type safety and optimize memory layout.
#[derive(Copy, Clone, PartialOrd, Ord)]
#[repr(transparent)]
pub struct MapId(NonMaxU32);

impl MapId {
    /// Sentinel value representing no table (invalid/empty ID).
    pub const EMPTY: MapId = MapId(NonMaxU32::ZERO);

    #[inline]
    pub(crate) const fn new(id: u32) -> Self {
        Self(NonMaxU32::new(id).expect("too many maps"))
    }

    /// Creates a new `MapId` from a usize.
    ///
    /// # Panics
    /// Panics if `id >= u32::MAX`.
    #[inline(always)]
    pub const fn without_provenance(id: usize) -> Self {
        if id >= u32::MAX as usize {
            voker_utils::cold_path();
            panic!("MapId must be < u32::MAX");
        }
        unsafe { Self(NonMaxU32::new_unchecked(id as u32)) }
    }

    /// Returns the map index as a usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }
}

impl Debug for MapId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for MapId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Hash for MapId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.0.get());
    }
}

impl PartialEq for MapId {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for MapId {}

// -----------------------------------------------------------------------------
// MapRow

/// Row position within a Map.
#[derive(Copy, Clone, PartialOrd, Ord)]
#[repr(transparent)]
pub struct MapRow(pub u32);

impl Debug for MapRow {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for MapRow {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Hash for MapRow {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.0);
    }
}

impl PartialEq for MapRow {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for MapRow {}
