use alloc::collections::BinaryHeap;
use core::alloc::Layout;
use core::cmp::Reverse;
use core::fmt::Debug;
use core::num::NonZeroUsize;

use voker_ptr::{OwningPtr, Ptr, PtrMut};
use voker_utils::hash::SparseHashMap;

use crate::borrow::{UntypedMut, UntypedRef};
use crate::entity::Entity;
use crate::storage::{AbortOnPanic, Column, MapRow};
use crate::tick::{CheckTicks, Tick};
use crate::utils::Dropper;

/// A mapping table from entities to component data.
///
/// `Map` manages the mapping from [`Entity`] to component data of a specific type.
/// It uses a [`Column`] as the underlying storage and maintains a [`SparseHashMap`]
/// for entity-to-location lookups.
///
/// # Storage Structure
/// - `column`: Stores the actual component data in a contiguous array
/// - `free`: A stack of available row indices for reuse
/// - `mapper`: Maps entities to their corresponding storage rows
pub struct Map {
    column: Column,
    free: BinaryHeap<Reverse<MapRow>>,
    capacity: usize,
    mapper: SparseHashMap<Entity, MapRow>,
}

unsafe impl Sync for Map {}
unsafe impl Send for Map {}

impl Debug for Map {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Map")
            .field("entities", &self.mapper.keys())
            .finish()
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        self.mapper.values().for_each(|v| unsafe {
            self.column.drop_item(v.0 as usize);
        });
        unsafe {
            self.column.dealloc(self.capacity);
        }
    }
}

impl Map {
    /// Creates a new `Map` with the specified component layout and drop function.
    pub(crate) fn new(layout: Layout, dropper: Option<Dropper>) -> Self {
        Self {
            column: unsafe { Column::new(layout, dropper) },
            free: BinaryHeap::new(),
            capacity: 0,
            mapper: SparseHashMap::new(),
        }
    }

    /// Allocates a new storage row for the given entity.
    ///
    /// This function either reuses a free row or reserves new memory when needed.
    ///
    /// # Safety
    /// - The entity must not already exist in the map
    /// - The returned `MapRow` is valid until explicitly removed
    #[inline]
    #[must_use]
    pub unsafe fn allocate(&mut self, entity: Entity) -> MapRow {
        #[cold]
        #[inline(never)]
        fn reserve_many(this: &mut Map) -> Reverse<MapRow> {
            let guard = AbortOnPanic;

            let new_cap = (this.capacity << 1).max(4);
            debug_assert!(new_cap <= u32::MAX as usize);

            let row = MapRow(this.capacity as u32);
            unsafe {
                let new_capacity = NonZeroUsize::new_unchecked(new_cap);
                if this.capacity == 0 {
                    this.column.alloc(new_capacity);
                } else {
                    let current = NonZeroUsize::new_unchecked(this.capacity);
                    this.column.realloc(current, new_capacity);
                }
            }

            let range = (this.capacity as u32 + 1)..(new_cap as u32);
            this.free.extend(range.map(MapRow).map(Reverse));

            this.capacity = new_cap;

            ::core::mem::forget(guard);
            Reverse(row)
        }

        debug_assert!(!self.mapper.contains_key(&entity));
        let row = self.free.pop().unwrap_or_else(|| reserve_many(self));
        self.mapper.insert(entity, row.0);
        row.0
    }

    /// Frees the storage row associated with an entity.
    ///
    /// This function:
    /// 1. Removes the entity from the mapping
    /// 2. Returns the associated row to the free pool for reuse
    /// 3. Does **not** drop or modify the component data
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - The component data at that row has been properly handled
    /// - The row will not be used after this call
    #[inline]
    #[must_use]
    pub unsafe fn deallocate(&mut self, entity: Entity) -> Option<MapRow> {
        let map_row = self.mapper.remove(&entity)?;
        self.free.push(Reverse(map_row));
        Some(map_row)
    }

    /// Gets the storage row for the given entity, if it exists.
    #[inline]
    pub fn get_map_row(&self, entity: Entity) -> Option<MapRow> {
        self.mapper.get(&entity).copied()
    }

    /// Gets a raw pointer to the component data at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `allocate` or `get_map_row`)
    /// - The caller must ensure proper synchronization when accessing the data
    #[inline(always)]
    pub unsafe fn get_data(&self, map_row: MapRow) -> Ptr<'_> {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.get_data(map_row.0 as usize) }
    }

    /// Gets a raw pointer to the component data at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `allocate` or `get_map_row`)
    /// - The caller must ensure proper synchronization when accessing the data
    #[inline(always)]
    pub unsafe fn get_data_mut(&mut self, map_row: MapRow) -> PtrMut<'_> {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.get_data_mut(map_row.0 as usize) }
    }

    /// Gets the tick when the component was added at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `allocate` or `get_map_row`)
    #[inline(always)]
    pub unsafe fn get_added(&self, map_row: MapRow) -> Tick {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.get_added(map_row.0 as usize) }
    }

    /// Gets the tick when the component was last changed at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `allocate` or `get_map_row`)
    #[inline(always)]
    pub unsafe fn get_changed(&self, map_row: MapRow) -> Tick {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.get_changed(map_row.0 as usize) }
    }

    /// Gets the tick when the component was added at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `allocate` or `get_map_row`)
    #[inline(always)]
    pub unsafe fn get_added_mut(&mut self, map_row: MapRow) -> &mut Tick {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.get_added_mut(map_row.0 as usize) }
    }

    /// Gets the tick when the component was last changed at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `allocate` or `get_map_row`)
    #[inline(always)]
    pub unsafe fn get_changed_mut(&mut self, map_row: MapRow) -> &mut Tick {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.get_added_mut(map_row.0 as usize) }
    }

    /// Gets an immutable reference to the component at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid
    /// - The caller must ensure that no mutable references exist to this data
    /// - The tick parameters must be consistent with the system scheduling
    #[inline(always)]
    pub unsafe fn get_ref(
        &self,
        map_row: MapRow,
        last_run: Tick,
        this_run: Tick,
    ) -> UntypedRef<'_> {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.get_ref(map_row.0 as usize, last_run, this_run) }
    }

    /// Gets a mutable reference to the component at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid
    /// - The caller must ensure that no other references exist to this data
    /// - The tick parameters must be consistent with the system scheduling
    #[inline(always)]
    pub unsafe fn get_mut(
        &mut self,
        map_row: MapRow,
        last_run: Tick,
        this_run: Tick,
    ) -> UntypedMut<'_> {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.get_mut(map_row.0 as usize, last_run, this_run) }
    }

    /// Initializes a new component at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid and uninitialized
    /// - The layout of `data` must match the column's layout
    #[inline]
    pub unsafe fn init_item(&mut self, map_row: MapRow, data: OwningPtr<'_>, tick: Tick) {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe {
            self.column.init_item(map_row.0 as usize, data, tick);
        }
    }

    /// Replaces an existing component at the specified row with new data.
    ///
    /// # Safety
    /// - `map_row` must be valid and initialized
    /// - The layout of `data` must match the column's layout
    #[inline]
    pub unsafe fn replace_item(&mut self, map_row: MapRow, data: OwningPtr<'_>, tick: Tick) {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe {
            self.column.replace_item(map_row.0 as usize, data, tick);
        }
    }

    /// Removes and returns the component data at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid and initialized
    /// - The caller is responsible for properly dropping the returned pointer
    /// - Call [`Map::deallocate`] before this function if the entity is removed.
    #[inline]
    #[must_use = "The returned pointer should be used."]
    pub unsafe fn remove_item(&mut self, map_row: MapRow) -> OwningPtr<'_> {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.remove_item(map_row.0 as usize) }
    }

    /// Drops the component data at the specified row without returning it.
    ///
    /// # Safety
    /// - `map_row` must be valid and initialized
    /// - Call [`Map::deallocate`] before this function if the entity is removed.
    #[inline]
    pub unsafe fn drop_item(&mut self, map_row: MapRow) {
        debug_assert!((map_row.0 as usize) < self.capacity);
        unsafe { self.column.drop_item(map_row.0 as usize) }
    }

    /// Updates tick information for all components in this map.
    ///
    /// This is used during change detection to update component access ticks.
    pub(crate) fn check_ticks(&mut self, check: CheckTicks) {
        if let Some(&row) = self.mapper.values().max() {
            unsafe {
                self.column.check_ticks(row.0 as usize, check);
            }
        }
    }
}
