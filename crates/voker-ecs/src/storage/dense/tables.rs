use alloc::boxed::Box;
use alloc::vec::Vec;
use core::{fmt::Debug, iter::FusedIterator};

use voker_utils::hash::HashMap;
use voker_utils::hash::hash_map::RawEntryMut;

use super::{Table, TableBuilder, TableId};
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
    pub(crate) tables: Vec<Table>,
    mapper: HashMap<Box<[ComponentId]>, TableId>,
}

impl Debug for Tables {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map().entries(self.tables.iter().enumerate()).finish()
    }
}

impl Tables {
    /// Creates a new empty table registry with the default empty table.
    #[inline]
    pub(crate) fn new() -> Self {
        let mut tables: Vec<Table> = Vec::new();
        let mut mapper: HashMap<Box<[ComponentId]>, TableId> = HashMap::new();

        tables.push(TableBuilder::new(0).build());
        mapper.insert(Box::new([]), TableId::EMPTY);

        Tables { tables, mapper }
    }

    /// Returns a reference to the table with the given ID, if it exists.
    #[inline(always)]
    pub fn get(&self, id: TableId) -> Option<&Table> {
        self.tables.get(id.index())
    }

    /// Returns a mutable reference to the table with the given ID, if it exists.
    #[inline(always)]
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

    /// Returns the ID of the table exactly matching the given component set, if any.
    #[inline]
    pub fn get_id(&self, components: &[ComponentId]) -> Option<TableId> {
        self.mapper.get(components).copied()
    }

    /// Returns an iterator over the tables.
    #[inline]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (TableId, &Table)> + FusedIterator {
        self.tables
            .iter()
            .enumerate()
            .map(|(id, table)| (TableId::new(id as u32), table))
    }

    /// Returns an iterator that allows modifying each table.
    #[inline]
    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = (TableId, &mut Table)> + FusedIterator {
        self.tables
            .iter_mut()
            .enumerate()
            .map(|(id, table)| (TableId::new(id as u32), table))
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
        idents: &[ComponentId],
    ) -> TableId {
        debug_assert!(idents.is_sorted());

        match self.mapper.raw_entry_mut().from_key(idents) {
            RawEntryMut::Occupied(entry) => *entry.get(),
            RawEntryMut::Vacant(entry) => {
                let table_id = TableId::new(self.tables.len() as u32);
                let mut builder = TableBuilder::new(idents.len());

                idents.iter().for_each(|&id| unsafe {
                    let info = components.get_unchecked(id);
                    builder.insert(id, info.layout(), info.dropper());
                });

                self.tables.push(builder.build());
                entry.insert(Box::from(idents), table_id);

                table_id
            }
        }
    }
}
