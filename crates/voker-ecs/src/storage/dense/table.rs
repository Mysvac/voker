use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::num::NonZeroUsize;
use core::panic::{RefUnwindSafe, UnwindSafe};

use voker_ptr::{OwningPtr, Ptr, PtrMut};

use super::{TableCol, TableId, TableRow};
use crate::borrow::{UntypedMut, UntypedRef};
use crate::borrow::{UntypedSliceMut, UntypedSliceRef};
use crate::component::{ComponentId, Components};
use crate::entity::{Entity, MovedEntityRow};
use crate::storage::{AbortOnPanic, Column, VecRemoveExt};
use crate::tick::Tick;

// -----------------------------------------------------------------------------
// Table

/// A dense columnar storage table for ECS components.
///
/// |  TableId  | Component A | Component B | Component C | .. |
/// |-----------|-------------|-------------|-------------|----|
/// | Entity A  | /* data */  | /* data */  | /* data */  | .. |
/// | Entity B  | /* data */  | /* data */  | /* data */  | .. |
/// | Entity C  | /* data */  | /* data */  | /* data */  | .. |
/// | ........  | ..........  | ..........  | ..........  | .. |
///
/// This structure provides optimal cache locality during iteration.
pub struct Table {
    id: TableId,
    columns: Box<[Column]>,
    compnents: &'static [ComponentId],
    entities: Vec<Entity>,
}

// -----------------------------------------------------------------------------
// Private

impl Table {
    pub(super) fn new(
        table_id: TableId,
        components: &Components,
        idents: &'static [ComponentId],
    ) -> Self {
        debug_assert!(idents.is_sorted());

        let mut columns: Vec<Column> = Vec::with_capacity(idents.len());
        idents.iter().for_each(|&id| {
            let info = unsafe { components.get_unchecked(id) };
            let layout = info.layout();
            let dropper = info.dropper();
            columns.push(unsafe { Column::new(layout, dropper) });
        });

        Self {
            id: table_id,
            columns: columns.into_boxed_slice(),
            compnents: idents,
            entities: Vec::new(),
        }
    }

    /// Updates change ticks for all components based on the provided check parameters.
    pub(crate) fn check_ticks(&mut self, now: Tick) {
        let len = self.entity_count();
        self.columns.iter_mut().for_each(|c| unsafe {
            c.check_ticks(len, now);
        });
    }

    /// Returns the current allocation capacity of the table.
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.entities.capacity()
    }

    /// Returns the number of entities currently stored in the table.
    #[inline(always)]
    fn entity_count(&self) -> usize {
        self.entities.len()
    }
}

// -----------------------------------------------------------------------------
// Basic

impl Debug for Table {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Table")
            .field("id", &self.id)
            .field("components", &self.compnents)
            .field("entities", &self.entities)
            .finish()
    }
}

impl Drop for Table {
    fn drop(&mut self) {
        let len = self.entity_count();
        let current_capacity = self.capacity();

        self.columns.iter_mut().for_each(|c| unsafe {
            c.drop_slice(len);
            c.dealloc(current_capacity);
        });
    }
}

unsafe impl Sync for Table {}
unsafe impl Send for Table {}
impl UnwindSafe for Table {}
impl RefUnwindSafe for Table {}

// -----------------------------------------------------------------------------
// Basic

impl Table {
    #[inline(always)]
    pub fn id(&self) -> TableId {
        self.id
    }

    #[inline(always)]
    pub fn components(&self) -> &[ComponentId] {
        self.compnents
    }

    #[inline(always)]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Allocates space for a new entity and returns its row index.
    ///
    /// # Safety
    /// - The entity must be unique within this table
    /// - The returned row is valid until the entity is removed
    #[must_use]
    pub unsafe fn alloc_row(&mut self, entity: Entity) -> TableRow {
        #[cold]
        #[inline(never)]
        fn reserve_many(this: &mut Table) {
            let abort_guard = AbortOnPanic;

            let old_cap = this.entities.capacity();
            this.entities.reserve(1);
            let new_cap = this.entities.capacity();

            assert!(new_cap <= u32::MAX as usize, "too many entities in a Table");

            unsafe {
                let new_capacity = NonZeroUsize::new_unchecked(new_cap);
                if let Some(current) = NonZeroUsize::new(old_cap) {
                    this.columns.iter_mut().for_each(|col| {
                        col.realloc(current, new_capacity);
                    });
                } else {
                    this.columns.iter_mut().for_each(|col| col.alloc(new_capacity));
                }
            }

            ::core::mem::forget(abort_guard);
        }

        let len = self.entities.len();
        if len == self.entities.capacity() {
            reserve_many(self);
        }

        self.entities.push(entity);
        // `0 < EntityId < u32::MAX`, so `len < u32::MAX`
        TableRow(len as u32)
    }

    /// Removes an entity by swapping with the last row and dropping its components.
    ///
    /// # Safety
    /// - `table_row` must be a valid, initialized row
    /// - After this operation, the row is no longer valid
    pub unsafe fn dealloc_row<const DROP: bool>(&mut self, table_row: TableRow) -> MovedEntityRow {
        let removal = table_row.0 as usize;
        let last = self.entity_count() - 1;
        debug_assert!(removal <= last);

        unsafe {
            if removal != last {
                let swapped = self.entities.move_last_to(last, removal);
                self.columns.iter_mut().for_each(|c| {
                    if DROP {
                        c.swap_drop_not_last(removal, last);
                    } else {
                        c.swap_forget_not_last(removal, last);
                    }
                });
                MovedEntityRow::in_table(Some(swapped), table_row)
            } else {
                voker_utils::cold_path();
                self.entities.set_len(last);
                if DROP {
                    self.columns.iter_mut().for_each(|c| {
                        c.drop_item(last);
                    });
                }
                MovedEntityRow::in_table(None, table_row)
            }
        }
    }

    /// Finds the column index for a given component ID using binary search.
    ///
    /// # Complexity
    /// O(log n) where n is the number of component types
    #[inline]
    pub fn get_table_col(&self, key: ComponentId) -> Option<TableCol> {
        let index = self.compnents.binary_search(&key).ok()?;
        Some(TableCol(index as u32))
    }

    /// Finds the row index for a given entity using linear search.
    ///
    /// # Complexity
    /// O(n) where n is the number of entities
    ///
    /// Note: This is inefficient and should be avoided. Store the `TableRow`
    /// returned by `allocate()` instead.
    #[inline]
    pub fn get_table_row(&self, key: Entity) -> Option<TableRow> {
        let index = self.entities.iter().position(|it| *it == key)?;
        Some(TableRow(index as u32))
    }

    /// Returns a reference to a column by its index.
    ///
    /// # Safety
    /// - `index` must be a valid column index obtained from `get_table_col()`
    #[inline(always)]
    pub unsafe fn get_column(&self, index: TableCol) -> &Column {
        debug_assert!((index.0 as usize) < self.columns.len());
        unsafe { self.columns.get_unchecked(index.0 as usize) }
    }

    /// Returns a mutable reference to a column by its index.
    ///
    /// # Safety
    /// - `index` must be a valid column index obtained from `get_table_col()`
    /// - No other references to the column may exist
    #[inline(always)]
    pub unsafe fn get_column_mut(&mut self, index: TableCol) -> &mut Column {
        debug_assert!((index.0 as usize) < self.columns.len());
        unsafe { self.columns.get_unchecked_mut(index.0 as usize) }
    }

    /// Returns a pointer to component data at the specified row and column.
    ///
    /// # Safety
    /// - `table_row` must be a valid row index
    /// - `table_col` must be a valid column index
    #[inline(always)]
    pub unsafe fn get_data(&self, table_row: TableRow, table_col: TableCol) -> Ptr<'_> {
        debug_assert!((table_row.0 as usize) < self.entity_count());
        unsafe {
            let col = self.get_column(table_col);
            col.get_data(table_row.0 as usize)
        }
    }

    /// Returns a pointer to component data at the specified row and column.
    ///
    /// # Safety
    /// - `table_row` must be a valid row index
    /// - `table_col` must be a valid column index
    #[inline(always)]
    pub unsafe fn get_data_mut(&mut self, table_row: TableRow, table_col: TableCol) -> PtrMut<'_> {
        debug_assert!((table_row.0 as usize) < self.entity_count());
        unsafe {
            let col = self.get_column_mut(table_col);
            col.get_data_mut(table_row.0 as usize)
        }
    }

    /// Returns the added tick for a component at the specified row and column.
    ///
    /// # Safety
    /// - `table_row` must be a valid row index
    /// - `table_col` must be a valid column index
    #[inline(always)]
    pub unsafe fn get_added(&self, table_row: TableRow, table_col: TableCol) -> Tick {
        debug_assert!((table_row.0 as usize) < self.entity_count());
        unsafe {
            let col = self.get_column(table_col);
            col.get_added(table_row.0 as usize)
        }
    }

    /// Returns the changed tick for a component at the specified row and column.
    ///
    /// # Safety
    /// - `table_row` must be a valid row index
    /// - `table_col` must be a valid column index
    #[inline(always)]
    pub unsafe fn get_changed(&self, table_row: TableRow, table_col: TableCol) -> Tick {
        debug_assert!((table_row.0 as usize) < self.entity_count());
        unsafe {
            let col = self.get_column(table_col);
            col.get_changed(table_row.0 as usize)
        }
    }

    /// Returns the added tick for a component at the specified row and column.
    ///
    /// # Safety
    /// - `table_row` must be a valid row index
    /// - `table_col` must be a valid column index
    #[inline(always)]
    pub unsafe fn get_added_mut(&mut self, table_row: TableRow, table_col: TableCol) -> &mut Tick {
        debug_assert!((table_row.0 as usize) < self.entity_count());
        unsafe {
            let col = self.get_column_mut(table_col);
            col.get_added_mut(table_row.0 as usize)
        }
    }

    /// Returns the changed tick for a component at the specified row and column.
    ///
    /// # Safety
    /// - `table_row` must be a valid row index
    /// - `table_col` must be a valid column index
    #[inline(always)]
    pub unsafe fn get_changed_mut(
        &mut self,
        table_row: TableRow,
        table_col: TableCol,
    ) -> &mut Tick {
        debug_assert!((table_row.0 as usize) < self.entity_count());
        unsafe {
            let col = self.get_column_mut(table_col);
            col.get_changed_mut(table_row.0 as usize)
        }
    }

    /// Returns a slice of added ticks for the entire column.
    ///
    /// # Safety
    /// - `table_col` must be a valid column index
    /// - The returned slice is only valid while the table is not mutated
    #[inline(always)]
    pub unsafe fn get_added_slice(&self, table_col: TableCol) -> &[Tick] {
        let len = self.entity_count();
        unsafe {
            let col = self.get_column(table_col);
            col.get_changed_slice().deref(len)
        }
    }

    /// Returns a slice of changed ticks for the entire column.
    ///
    /// # Safety
    /// - `table_col` must be a valid column index
    /// - The returned slice is only valid while the table is not mutated
    #[inline(always)]
    pub unsafe fn get_changed_slice(&self, table_col: TableCol) -> &[Tick] {
        let len = self.entity_count();
        unsafe {
            let col = self.get_column(table_col);
            col.get_changed_slice().deref(len)
        }
    }

    /// Returns an untyped reference to a component with change tracking.
    ///
    /// # Safety
    /// - `table_row` and `table_col` must be valid
    /// - The component must be initialized at the given row
    #[inline(always)]
    pub unsafe fn get_ref(
        &self,
        table_row: TableRow,
        table_col: TableCol,
        last_run: Tick,
        this_run: Tick,
    ) -> UntypedRef<'_> {
        debug_assert!((table_row.0 as usize) < self.entity_count());
        unsafe {
            let col = self.get_column(table_col);
            col.get_ref(table_row.0 as usize, last_run, this_run)
        }
    }

    /// Returns an untyped mutable reference to a component with change tracking.
    ///
    /// # Safety
    /// - `table_row` and `table_col` must be valid
    /// - The component must be initialized at the given row
    /// - No other references to the component may exist
    #[inline(always)]
    pub unsafe fn get_mut(
        &mut self,
        table_row: TableRow,
        table_col: TableCol,
        last_run: Tick,
        this_run: Tick,
    ) -> UntypedMut<'_> {
        debug_assert!((table_row.0 as usize) < self.entity_count());
        unsafe {
            let col = self.get_column_mut(table_col);
            col.get_mut(table_row.0 as usize, last_run, this_run)
        }
    }

    /// Returns an untyped slice reference to an entire column with change tracking.
    ///
    /// # Safety
    /// - `table_col` must be a valid column index
    /// - All components in the column must be initialized
    #[inline(always)]
    pub unsafe fn get_slice_ref(
        &self,
        table_col: TableCol,
        last_run: Tick,
        this_run: Tick,
    ) -> UntypedSliceRef<'_> {
        let len = self.entity_count();
        unsafe {
            let col = self.get_column(table_col);
            col.get_slice_ref(len, last_run, this_run)
        }
    }

    /// Returns an untyped mutable slice reference to an entire column with change tracking.
    ///
    /// # Safety
    /// - `table_col` must be a valid column index
    /// - All components in the column must be initialized
    /// - No other references to the column may exist
    #[inline(always)]
    pub unsafe fn get_slice_mut(
        &mut self,
        table_col: TableCol,
        last_run: Tick,
        this_run: Tick,
    ) -> UntypedSliceMut<'_> {
        let len = self.entity_count();
        unsafe {
            let col = self.get_column_mut(table_col);
            col.get_slice_mut(len, last_run, this_run)
        }
    }

    /// Initializes a component at the specified row.
    ///
    /// # Safety
    /// - `table_row` and `table_col` must be valid
    /// - The component slot must be uninitialized
    /// - `data` must point to valid data matching the column's type
    #[inline]
    pub unsafe fn init_item(
        &mut self,
        table_col: TableCol,
        table_row: TableRow,
        data: OwningPtr<'_>,
        tick: Tick,
    ) {
        debug_assert!((table_row.0 as usize) < self.entity_count());

        unsafe {
            let column = self.get_column_mut(table_col);
            column.init_item(table_row.0 as usize, data, tick);
        }
    }

    /// Replaces an existing component at the specified row.
    ///
    /// # Safety
    /// - `table_row` and `table_col` must be valid
    /// - The component slot must be initialized
    /// - `data` must point to valid data matching the column's type
    #[inline]
    pub unsafe fn replace_item(
        &mut self,
        table_col: TableCol,
        table_row: TableRow,
        data: OwningPtr<'_>,
        tick: Tick,
    ) {
        debug_assert!((table_row.0 as usize) < self.entity_count());

        unsafe {
            let column = self.get_column_mut(table_col);
            column.replace_item(table_row.0 as usize, data, tick);
        }
    }

    /// Removes a component and returns ownership of its data.
    ///
    /// # Safety
    /// - `table_row` and `table_col` must be valid
    /// - The component slot must be initialized
    /// - Caller must ensure the returned `OwningPtr` is properly handled
    #[inline]
    #[must_use = "The returned pointer should be used."]
    pub unsafe fn remove_item(
        &mut self,
        table_col: TableCol,
        table_row: TableRow,
    ) -> OwningPtr<'_> {
        debug_assert!((table_row.0 as usize) < self.entity_count());

        unsafe {
            let column = self.get_column_mut(table_col);
            column.remove_item(table_row.0 as usize)
        }
    }

    /// Drops the component data at the specified location without returning it.
    ///
    /// # Safety
    /// - `table_row` and `table_col` must be valid
    /// - The component slot must be initialized
    /// - Caller must ensure the returned `OwningPtr` is properly handled
    #[inline]
    pub unsafe fn drop_item(&mut self, table_col: TableCol, table_row: TableRow) {
        debug_assert!((table_row.0 as usize) < self.entity_count());

        unsafe {
            let column = self.get_column_mut(table_col);
            column.drop_item(table_row.0 as usize)
        }
    }
}

// -----------------------------------------------------------------------------
// Move, Init data

impl Table {
    /// Moves an entity to another table.
    ///
    /// # Safety
    /// - `table_row` must be a valid, initialized row in this table
    /// - `other` must be a valid table
    /// - Components are properly moved or dropped based on presence in destination
    pub unsafe fn move_row<const DROP: bool>(
        &mut self,
        table_row: TableRow,
        other: &mut Table,
    ) -> (MovedEntityRow, TableRow) {
        let src = table_row.0 as usize;
        let last = self.entity_count() - 1;
        debug_assert!(src <= last);

        unsafe {
            if src != last {
                let moved = *self.entities.get_unchecked(src);
                let swapped = self.entities.move_last_to(last, src);
                let new_row = other.alloc_row(moved);
                let dst = new_row.0 as usize;

                self.compnents
                    .iter()
                    .zip(self.columns.iter_mut())
                    .for_each(|(&id, col)| {
                        if let Some(table_col) = other.get_table_col(id) {
                            let other_col = other.get_column_mut(table_col);
                            col.move_item_to(other_col, src, dst);
                            col.swap_forget_not_last(src, last);
                        } else if DROP {
                            col.swap_drop_not_last(src, last);
                        } else {
                            col.swap_forget_not_last(src, last);
                        }
                    });

                (MovedEntityRow::in_table(Some(swapped), table_row), new_row)
            } else {
                voker_utils::cold_path();
                let moved = self.entities.remove_last(last);
                let new_row = other.alloc_row(moved);
                let dst = new_row.0 as usize;

                self.compnents
                    .iter()
                    .zip(self.columns.iter_mut())
                    .for_each(|(&id, col)| {
                        if let Some(table_col) = other.get_table_col(id) {
                            let other_col = other.get_column_mut(table_col);
                            col.move_item_to(other_col, src, dst);
                        } else if DROP {
                            col.drop_item(last);
                        }
                    });

                (MovedEntityRow::in_table(None, table_row), new_row)
            }
        }
    }
}
