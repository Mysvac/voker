use alloc::vec::Vec;
use core::fmt::Debug;

use voker_utils::hash::HashMap;
use voker_utils::hash::map::Entry;

use super::{Table, TableId};
use crate::component::{ComponentId, ComponentInfo, Components};

// -----------------------------------------------------------------------------
// Tables

/// Central registry managing all tables in the ECS storage.
///
/// Maintains:
/// - A vector of all tables
/// - A precise map from component sets to table IDs (for exact matches)
/// - A rough index for fast filtering by component presence
pub struct Tables {
    tables: Vec<Table>,
    mapper: HashMap<&'static [ComponentId], TableId>,
}

// -----------------------------------------------------------------------------
// Private

impl Tables {
    /// Creates a new empty table registry with the default empty table.
    pub(crate) fn new() -> Self {
        let mut val = Self {
            tables: Vec::new(),
            mapper: HashMap::new(),
        };

        let table = Table::new(TableId::EMPTY, &Components::new(), &[]);
        val.tables.push(table);
        val.mapper.insert(&[], TableId::EMPTY);

        val
    }

    /// Prepares the rough index for a new component type.
    #[inline(always)]
    pub(crate) fn prepare(&mut self, _info: &ComponentInfo) {
        // nothing
    }

    /// Registers a new table with the given component set, or returns an existing one.
    ///
    /// # Safety
    /// - `idents` must be sorted and contain valid component IDs
    /// - All component infos must be accessible from `components`
    pub(crate) unsafe fn register(
        &mut self,
        components: &Components,
        idents: &'static [ComponentId],
    ) -> TableId {
        debug_assert!(idents.is_sorted());

        match self.mapper.entry(idents) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let table_id = TableId::new(self.tables.len() as u32);
                let table = Table::new(table_id, components, idents);
                self.tables.push(table);
                entry.insert(table_id);

                table_id
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Tables

impl Debug for Tables {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(self.tables.as_slice(), f)
    }
}

impl Tables {
    /// Returns the number of registered tables.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "len > 0")]
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// Returns the ID of the table exactly matching the given component set, if any.
    ///
    /// The component slice must use the same canonical ordering used during
    /// table registration.
    #[inline]
    pub fn get_id(&self, components: &[ComponentId]) -> Option<TableId> {
        self.mapper.get(components).copied()
    }

    /// Returns a reference to the table with the given ID, if it exists.
    #[inline]
    pub fn get(&self, id: TableId) -> Option<&Table> {
        self.tables.get(id.index())
    }

    /// Returns a mutable reference to the table with the given ID, if it exists.
    #[inline]
    pub fn get_mut(&mut self, id: TableId) -> Option<&mut Table> {
        self.tables.get_mut(id.index())
    }

    /// Returns a reference to the table with the given ID without bounds checking.
    ///
    /// # Safety
    /// - `id` must be a valid table ID obtained from this registry
    /// - The table must not be concurrently accessed mutably
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, id: TableId) -> &Table {
        debug_assert!(id.index() < self.tables.len());
        unsafe { self.tables.get_unchecked(id.index()) }
    }

    /// Returns a mutable reference to the table with the given ID without bounds checking.
    ///
    /// # Safety
    /// - `id` must be a valid table ID obtained from this registry
    /// - No other references to the table may exist
    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, id: TableId) -> &mut Table {
        debug_assert!(id.index() < self.tables.len());
        unsafe { self.tables.get_unchecked_mut(id.index()) }
    }

    /// Returns all tables as a shared slice.
    #[inline]
    pub fn as_slice(&self) -> &[Table] {
        self.tables.as_slice()
    }

    /// Returns all tables as a mutable slice.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Table] {
        self.tables.as_mut_slice()
    }

    /// Returns an iterator over the tables.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, Table> {
        self.tables.iter()
    }

    /// Returns an iterator that allows modifying each table.
    #[inline]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, Table> {
        self.tables.iter_mut()
    }
}
