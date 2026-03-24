use core::fmt::Debug;

use voker_utils::num::NonMaxU32;

use crate::archetype::ArcheId;
use crate::storage::TableId;

/// A union type that can represent either a table ID or an archetype ID.
///
/// `StorageId` provides a space-efficient representation for cases where
/// we need to store either a [`TableId`] or an [`ArcheId`] interchangeably.
/// Both ID types are backed by `NonMaxU32`, allowing this union to pack
/// them into a single 32-bit value.
///
/// # Memory Layout
/// - Size: 32 bits (same as `u32`)
/// - Alignment: 4 bytes
/// - Both variants occupy the same memory location
///
/// # Safety
/// This type uses a union to achieve zero-cost abstraction. Users must
/// be careful to read from the correct variant that was last written.
#[derive(Clone, Copy)]
pub union StorageId {
    pub table_id: TableId,
    pub arche_id: ArcheId,
}

impl StorageId {
    const _STATIC_ASSERT_: () = const {
        let table = StorageId {
            table_id: TableId::new(12345),
        };
        let arche = StorageId {
            arche_id: ArcheId::new(12345),
        };
        assert!(table.get() == 12345);
        assert!(arche.get() == 12345);
    };

    #[inline(always)]
    const fn get(self) -> u32 {
        unsafe { core::mem::transmute::<StorageId, NonMaxU32>(self).get() }
    }

    #[inline(always)]
    const fn bits(self) -> u32 {
        unsafe { core::mem::transmute::<StorageId, u32>(self) }
    }
}

impl PartialEq for StorageId {
    fn eq(&self, other: &Self) -> bool {
        // `bits` is faster than `get`.
        self.bits() == other.bits()
    }
}

impl Eq for StorageId {}

impl PartialOrd for StorageId {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(Ord::cmp(self, other))
    }
}

impl Ord for StorageId {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // We must use representation values to ensure
        // that the sorting results remain in ascending order.
        Ord::cmp(&self.get(), &other.get())
    }
}

impl Debug for StorageId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.get(), f)
    }
}
