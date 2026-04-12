use alloc::collections::BinaryHeap;
use alloc::vec::Vec;
use core::cmp::Reverse;
use core::fmt::Debug;
use core::iter::FusedIterator;
use core::num::NonZeroUsize;
use core::panic::{RefUnwindSafe, UnwindSafe};

use voker_ptr::{OwningPtr, Ptr, PtrMut};
use voker_utils::hash::SparseHashMap;

use super::{MapId, MapRow};
use crate::borrow::{UntypedMut, UntypedRef};
use crate::component::{ComponentId, ComponentInfo};
use crate::entity::Entity;
use crate::storage::{AbortOnPanic, Column};
use crate::tick::Tick;
use crate::utils::DebugCheckedUnwrap;

// -----------------------------------------------------------------------------
// Map

/// A mapping table from entities to component data.
///
/// `Map` manages the mapping from [`Entity`] to component data of a specific type.
///
/// It uses a [`Column`] as the underlying storage and maintains a `HashMap`
/// for entity-to-location lookups.
pub struct Map {
    id: MapId,
    component: ComponentId,
    entities: Vec<Option<Entity>>,
    free: BinaryHeap<Reverse<MapRow>>,
    mapper: SparseHashMap<Entity, MapRow>,
    column: Column,
}

// -----------------------------------------------------------------------------
// Private

impl Map {
    /// Creates a new `Map` with the specified component layout and drop function.
    pub(crate) fn new(map_id: MapId, info: &ComponentInfo) -> Self {
        let id = info.id();
        let layout = info.layout();
        let dropper = info.dropper();

        Self {
            id: map_id,
            component: id,
            column: unsafe { Column::new(layout, dropper) },
            free: BinaryHeap::new(),
            entities: Vec::new(),
            mapper: SparseHashMap::new(),
        }
    }

    /// Updates tick information for all components in this map.
    ///
    /// This is used during change detection to update component access ticks.
    pub(crate) fn check_ticks(&mut self, now: Tick) {
        if let Some(&row) = self.mapper.values().max() {
            unsafe {
                self.column.check_ticks(row.0 as usize, now);
            }
        }
    }

    /// Returns the current allocation capacity of the table.
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.entities.capacity()
    }
}

// -----------------------------------------------------------------------------
// Basic

impl Debug for Map {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Map")
            .field("id", &self.id)
            .field("component", &self.component)
            .field("entities", &self.mapper.keys())
            .finish()
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        let current_capacity = self.capacity();

        for (idx, v) in self.entities.iter().enumerate() {
            if v.is_some() {
                unsafe {
                    self.column.drop_item(idx);
                }
            }
        }

        unsafe {
            self.column.dealloc(current_capacity);
        }
    }
}

unsafe impl Sync for Map {}
unsafe impl Send for Map {}
impl UnwindSafe for Map {}
impl RefUnwindSafe for Map {}

// -----------------------------------------------------------------------------
// Basic

impl Map {
    /// Returns this sparse map's identifier.
    #[inline]
    pub fn id(&self) -> MapId {
        self.id
    }

    /// Returns the component type identifier stored by this map.
    #[inline]
    pub fn component(&self) -> ComponentId {
        self.component
    }

    /// Iterates all entities that currently have this sparse component.
    #[inline]
    pub fn entities(&self) -> impl ExactSizeIterator + FusedIterator<Item = Entity> + '_ {
        self.mapper.keys().copied()
    }

    /// Allocates a new storage row for the given entity.
    ///
    /// This function either reuses a free row or reserves new memory when needed.
    ///
    /// # Safety
    /// - The entity must not already exist in the map
    /// - The returned `MapRow` is valid until explicitly removed
    #[must_use]
    pub unsafe fn alloc_row(&mut self, entity: Entity) -> MapRow {
        #[cold]
        #[inline(never)]
        fn reserve_many(this: &mut Map) -> Reverse<MapRow> {
            let guard = AbortOnPanic;

            let old_cap = this.entities.capacity();
            this.entities.reserve(1);
            let new_cap = this.entities.capacity();
            this.entities.resize(new_cap, const { None });

            assert!(new_cap <= u32::MAX as usize, "too many entities in a Map");

            unsafe {
                let new_capacity = NonZeroUsize::new_unchecked(new_cap);
                if let Some(current) = NonZeroUsize::new(old_cap) {
                    this.column.realloc(current, new_capacity);
                } else {
                    this.column.alloc(new_capacity);
                }
            }

            let range = (old_cap as u32 + 1)..(new_cap as u32);
            this.free.extend(range.map(MapRow).map(Reverse));

            ::core::mem::forget(guard);
            Reverse(MapRow(old_cap as u32))
        }

        let row = self.free.pop().unwrap_or_else(|| reserve_many(self)).0;
        unsafe {
            *self.entities.get_unchecked_mut(row.0 as usize) = Some(entity);
            self.mapper.insert(entity, row);
        }

        row
    }

    /// Deallocates a map row and optionally drops the stored component value.
    ///
    /// # Safety
    /// - `map_row` must reference a live row in this map
    /// - If `DROP` is `false`, caller is responsible for value-drop semantics
    pub unsafe fn dealloc_row<const DROP: bool>(&mut self, map_row: MapRow) {
        let removal = map_row.0 as usize;
        debug_assert!(removal < self.capacity());
        let entity = unsafe { self.entities.get_unchecked_mut(removal).take() };
        let entity = unsafe { entity.debug_checked_unwrap() };
        let removed = self.mapper.remove(&entity);
        debug_assert_eq!(removed, Some(map_row));
        self.free.push(Reverse(map_row));

        if DROP {
            unsafe {
                self.drop_item(map_row);
            }
        }
    }

    /// Gets the storage row for the given entity, if it exists.
    #[inline]
    pub fn get_map_row(&self, entity: Entity) -> Option<MapRow> {
        self.mapper.get(&entity).copied()
    }

    /// Gets a raw pointer to the component data at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `alloc_row` or `get_map_row`)
    /// - The caller must ensure proper synchronization when accessing the data
    #[inline(always)]
    pub unsafe fn get_data(&self, map_row: MapRow) -> Ptr<'_> {
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.get_data(map_row.0 as usize) }
    }

    /// Gets a raw pointer to the component data at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `alloc_row` or `get_map_row`)
    /// - The caller must ensure proper synchronization when accessing the data
    #[inline(always)]
    pub unsafe fn get_data_mut(&mut self, map_row: MapRow) -> PtrMut<'_> {
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.get_data_mut(map_row.0 as usize) }
    }

    /// Gets the tick when the component was added at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `alloc_row` or `get_map_row`)
    #[inline(always)]
    pub unsafe fn get_added(&self, map_row: MapRow) -> Tick {
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.get_added(map_row.0 as usize) }
    }

    /// Gets the tick when the component was last changed at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `alloc_row` or `get_map_row`)
    #[inline(always)]
    pub unsafe fn get_changed(&self, map_row: MapRow) -> Tick {
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.get_changed(map_row.0 as usize) }
    }

    /// Gets the tick when the component was added at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `alloc_row` or `get_map_row`)
    #[inline(always)]
    pub unsafe fn get_added_mut(&mut self, map_row: MapRow) -> &mut Tick {
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.get_added_mut(map_row.0 as usize) }
    }

    /// Gets the tick when the component was last changed at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid (obtained from `alloc_row` or `get_map_row`)
    #[inline(always)]
    pub unsafe fn get_changed_mut(&mut self, map_row: MapRow) -> &mut Tick {
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.get_changed_mut(map_row.0 as usize) }
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
        debug_assert!((map_row.0 as usize) < self.capacity());
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
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.get_mut(map_row.0 as usize, last_run, this_run) }
    }

    /// Initializes a new component at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid and uninitialized
    /// - The layout of `data` must match the column's layout
    #[inline]
    pub unsafe fn init_item(&mut self, map_row: MapRow, data: OwningPtr<'_>, tick: Tick) {
        debug_assert!((map_row.0 as usize) < self.capacity());
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
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe {
            self.column.replace_item(map_row.0 as usize, data, tick);
        }
    }

    /// Removes and returns the component data at the specified row.
    ///
    /// # Safety
    /// - `map_row` must be valid and initialized
    /// - The caller is responsible for properly dropping the returned pointer
    #[inline]
    #[must_use = "The returned pointer should be used."]
    pub unsafe fn remove_item(&mut self, map_row: MapRow) -> OwningPtr<'_> {
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.remove_item(map_row.0 as usize) }
    }

    /// Drops the component data at the specified row without returning it.
    ///
    /// # Safety
    /// - `map_row` must be valid and initialized
    #[inline]
    pub unsafe fn drop_item(&mut self, map_row: MapRow) {
        debug_assert!((map_row.0 as usize) < self.capacity());
        unsafe { self.column.drop_item(map_row.0 as usize) }
    }
}
