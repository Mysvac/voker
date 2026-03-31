use core::ptr::NonNull;

use alloc::vec::Vec;

use super::{QueryData, ReadOnlyQueryData};
use crate::archetype::Archetype;
use crate::component::{Component, ComponentId, ComponentStorage};
use crate::entity::Entity;
use crate::storage::{Column, Map, Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// DataView / ComponentView

pub union DataView {
    dense: Option<NonNull<Column>>,
    sparse: Option<NonNull<Map>>,
}

impl DataView {
    const fn build_dense() -> Self {
        DataView { dense: None }
    }

    fn build_sparse(component: ComponentId, world: UnsafeWorld) -> Self {
        let world_ref = unsafe { world.read_only() };
        let maps = &world_ref.storages.maps;
        let Some(map_id) = maps.get_id(component) else {
            return DataView { sparse: None };
        };
        let map = unsafe { maps.get_unchecked(map_id) };
        DataView {
            sparse: Some(NonNull::from_ref(map)),
        }
    }

    fn update_dense(&mut self, component: ComponentId, table: &Table) {
        if let Some(table_col) = table.get_table_col(component) {
            let column = unsafe { table.get_column(table_col) };
            self.dense = Some(NonNull::from_ref(column));
        } else {
            self.dense = None;
        };
    }
}

pub struct ComponentView {
    data: DataView,
    this_run: Tick,
}

impl ComponentView {
    fn build_dense(this_run: Tick) -> Self {
        ComponentView {
            this_run,
            data: DataView { dense: None },
        }
    }

    fn build_sparse(component: ComponentId, world: UnsafeWorld, this_run: Tick) -> Self {
        ComponentView {
            this_run,
            data: DataView::build_sparse(component, world),
        }
    }

    fn update_dense(&mut self, component: ComponentId, table: &Table) {
        self.data.update_dense(component, table);
    }
}

// -----------------------------------------------------------------------------
// &T

unsafe impl<T: Component> ReadOnlyQueryData for &T {}

unsafe impl<T: Component> QueryData for &T {
    type State = ComponentId;
    type Cache<'world> = DataView;
    type Item<'world> = &'world T;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Cache<'w> {
        match T::STORAGE {
            ComponentStorage::Dense => DataView::build_dense(),
            ComponentStorage::Sparse => DataView::build_sparse(*state, world),
        }
    }

    fn build_filter(state: &Self::State, out: &mut Vec<FilterParamBuilder>) {
        out.iter_mut().for_each(|param| {
            param.with(*state);
        });
    }

    fn build_access(state: &Self::State, out: &mut AccessParam) -> bool {
        out.set_reading(*state)
    }

    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        _arche: &'w Archetype,
        table: &'w Table,
    ) {
        if T::STORAGE.is_dense() {
            cache.update_dense(*state, table);
        }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        if T::STORAGE.is_dense() {
            cache.update_dense(*state, table);
        }
    }

    unsafe fn fetch<'w>(
        _state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w>> {
        match T::STORAGE {
            ComponentStorage::Dense => {
                let ptr = unsafe { cache.dense }?;
                let column = unsafe { &*ptr.as_ptr() };
                let row = table_row.0 as usize;
                let data = unsafe { column.get_data(row) };
                data.debug_assert_aligned::<T>();
                Some(unsafe { data.deref::<T>() })
            }
            ComponentStorage::Sparse => {
                let ptr = unsafe { cache.sparse }?;
                let map = unsafe { &*ptr.as_ptr() };
                let row = map.get_map_row(entity)?;
                let ptr = unsafe { map.get_data(row) };
                ptr.debug_assert_aligned::<T>();
                Some(unsafe { ptr.deref::<T>() })
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Option<&T>

unsafe impl<T: Component> ReadOnlyQueryData for Option<&T> {}

unsafe impl<T: Component> QueryData for Option<&T> {
    type State = ComponentId;
    type Cache<'world> = DataView;
    type Item<'world> = Option<&'world T>;

    // Due to `Option`, this data will not affect the filter.
    const COMPONENTS_ARE_DENSE: bool = false;

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        unsafe { <&T as QueryData>::build_cache(state, world, last_run, this_run) }
    }

    fn build_filter(_state: &Self::State, _out: &mut Vec<FilterParamBuilder>) {
        // Because `Option`, we do not set filter.
    }

    fn build_access(state: &Self::State, out: &mut AccessParam) -> bool {
        <&T as QueryData>::build_access(state, out)
    }

    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        arche: &'w Archetype,
        table: &'w Table,
    ) {
        unsafe {
            <&T as QueryData>::set_for_arche(state, cache, arche, table);
        }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        unsafe {
            <&T as QueryData>::set_for_table(state, cache, table);
        }
    }

    unsafe fn fetch<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w>> {
        Some(unsafe { <&T as QueryData>::fetch(state, cache, entity, table_row) })
    }
}

// -----------------------------------------------------------------------------
// &mut T

unsafe impl<T: Component> QueryData for &mut T {
    type State = ComponentId;
    type Cache<'world> = ComponentView;
    type Item<'world> = &'world mut T;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        _last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        match T::STORAGE {
            ComponentStorage::Dense => ComponentView::build_dense(this_run),
            ComponentStorage::Sparse => ComponentView::build_sparse(*state, world, this_run),
        }
    }

    fn build_filter(state: &Self::State, out: &mut Vec<FilterParamBuilder>) {
        out.iter_mut().for_each(|param| {
            param.with(*state);
        });
    }

    fn build_access(state: &Self::State, out: &mut AccessParam) -> bool {
        out.set_writing(*state)
    }

    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        _arche: &'w Archetype,
        table: &'w Table,
    ) {
        if T::STORAGE.is_dense() {
            cache.update_dense(*state, table);
        }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        if T::STORAGE.is_dense() {
            cache.update_dense(*state, table);
        }
    }

    unsafe fn fetch<'w>(
        _state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w>> {
        match T::STORAGE {
            ComponentStorage::Dense => {
                let ptr = unsafe { cache.data.dense }?;
                let column = unsafe { &mut *ptr.as_ptr() };
                let row = table_row.0 as usize;
                unsafe {
                    *column.get_changed_mut(row) = cache.this_run;
                }
                let data = unsafe { column.get_data_mut(row) };
                data.debug_assert_aligned::<T>();
                Some(unsafe { data.deref::<T>() })
            }
            ComponentStorage::Sparse => {
                let ptr = unsafe { cache.data.sparse }?;
                let map = unsafe { &mut *ptr.as_ptr() };
                let row = map.get_map_row(entity)?;
                unsafe {
                    *map.get_changed_mut(row) = cache.this_run;
                }
                let ptr = unsafe { map.get_data_mut(row) };
                ptr.debug_assert_aligned::<T>();
                Some(unsafe { ptr.deref::<T>() })
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Option<&mut T>

unsafe impl<T: Component> QueryData for Option<&mut T> {
    type State = ComponentId;
    type Cache<'world> = ComponentView;
    type Item<'world> = Option<&'world mut T>;

    // Due to `Option`, this data will not affect the filter.
    const COMPONENTS_ARE_DENSE: bool = false;

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        unsafe { <&mut T as QueryData>::build_cache(state, world, last_run, this_run) }
    }

    fn build_filter(_state: &Self::State, _out: &mut Vec<FilterParamBuilder>) {
        // Because `Option`, we do not set filter.
    }

    fn build_access(state: &Self::State, out: &mut AccessParam) -> bool {
        <&mut T as QueryData>::build_access(state, out)
    }

    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        arche: &'w Archetype,
        table: &'w Table,
    ) {
        unsafe {
            <&mut T as QueryData>::set_for_arche(state, cache, arche, table);
        }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        unsafe {
            <&mut T as QueryData>::set_for_table(state, cache, table);
        }
    }

    unsafe fn fetch<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w>> {
        Some(unsafe { <&mut T as QueryData>::fetch(state, cache, entity, table_row) })
    }
}
