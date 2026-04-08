use core::fmt::{Debug, Display};
use core::hash::Hash;

use voker_utils::num::NonMaxU32;

// -----------------------------------------------------------------------------
// TableId

/// Unique identifier for a table in the ECS storage.
///
/// `TableId` is an index for Table, and also represents
/// a combination of dense components.
///
/// Wraps a non-max u32 to provide type safety and optimize memory layout.
#[derive(Copy, Clone, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TableId(NonMaxU32);

impl TableId {
    /// Sentinel value representing a empty table (no compoennts).
    pub const EMPTY: TableId = TableId(NonMaxU32::ZERO);

    /// # Panics
    /// Panics if `id` would cause the maximum number of tables to be exceeded.
    #[inline]
    pub(crate) const fn new(id: u32) -> Self {
        Self(NonMaxU32::new(id).expect("too many tables"))
    }

    /// # Safety
    /// The value must not be the maximum value of the underlying integer type.
    #[inline(always)]
    pub(crate) const unsafe fn new_unchecked(id: u32) -> Self {
        Self(unsafe { NonMaxU32::new_unchecked(id) })
    }

    /// Creates a new `TableId` from a usize.
    ///
    /// # Panics
    /// Panics if `id >= u32::MAX`.
    #[inline(always)]
    pub const fn without_provenance(id: usize) -> Self {
        if id >= u32::MAX as usize {
            voker_utils::cold_path();
            panic!("TableId must be < u32::MAX");
        }
        unsafe { Self(NonMaxU32::new_unchecked(id as u32)) }
    }

    /// Returns the table index as a usize.
    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0.get() as usize
    }
}

impl Debug for TableId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for TableId {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Hash for TableId {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        // Sparse hashing is optimized for smaller values.
        // So we use represented values, rather than the underlying bits
        state.write_u32(self.0.get());
    }
}

impl PartialEq for TableId {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for TableId {}

// -----------------------------------------------------------------------------
// TableRow

/// Row position within a table.
///
/// Represents an index into a table's columnar storage.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TableRow(pub u32);

impl Debug for TableRow {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for TableRow {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Hash for TableRow {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.0);
    }
}

// -----------------------------------------------------------------------------
// TableCol

/// Column position within a table.
///
/// Represents an index into a table's columnar storage for a specific component type.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct TableCol(pub u32);

impl Debug for TableCol {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for TableCol {
    #[inline(always)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl Hash for TableCol {
    #[inline(always)]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_u32(self.0);
    }
}
