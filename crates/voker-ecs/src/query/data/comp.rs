use core::ptr::NonNull;

use alloc::vec::Vec;

use super::{QueryData, ReadOnlyQueryData};
use crate::archetype::Archetype;
use crate::borrow::{Mut, Ref};
use crate::component::{Component, ComponentId, StorageMode};
use crate::entity::Entity;
use crate::storage::{Column, Map, Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// ComponentView

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
    last_run: Tick,
    this_run: Tick,
}

impl ComponentView {
    fn build_dense(last_run: Tick, this_run: Tick) -> Self {
        ComponentView {
            last_run,
            this_run,
            data: DataView { dense: None },
        }
    }

    fn build_sparse(
        component: ComponentId,
        world: UnsafeWorld,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        let world_ref = unsafe { world.read_only() };
        let maps = &world_ref.storages.maps;
        if let Some(map_id) = maps.get_id(component) {
            let map = unsafe { maps.get_unchecked(map_id) };
            ComponentView {
                last_run,
                this_run,
                data: DataView {
                    sparse: Some(NonNull::from_ref(map)),
                },
            }
        } else {
            ComponentView {
                last_run,
                this_run,
                data: DataView { sparse: None },
            }
        }
    }

    fn update_dense(&mut self, component: ComponentId, table: &Table) {
        if let Some(table_col) = table.get_table_col(component) {
            let column = unsafe { table.get_column(table_col) };
            self.data.dense = Some(NonNull::from_ref(column));
        } else {
            self.data.dense = None;
        };
    }
}

// -----------------------------------------------------------------------------
// &T

unsafe impl<T: Component> ReadOnlyQueryData for &T {}

unsafe impl<T: Component> QueryData for &T {
    type ReadOnly = Self;
    type State = ComponentId;
    type Cache<'world> = DataView;
    type Item<'world> = &'world T;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn try_build_state(world: &World) -> Option<Self::State> {
        world.get_component_id::<T>()
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Cache<'w> {
        match T::STORAGE {
            StorageMode::Dense => DataView::build_dense(),
            StorageMode::Sparse => DataView::build_sparse(*state, world),
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
            StorageMode::Dense => {
                let ptr = unsafe { cache.dense }?;
                let column = unsafe { &*ptr.as_ptr() };
                let row = table_row.0 as usize;
                let data = unsafe { column.get_data(row) };
                data.debug_assert_aligned::<T>();
                Some(unsafe { data.deref::<T>() })
            }
            StorageMode::Sparse => {
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
    type ReadOnly = Self;
    type State = ComponentId;
    type Cache<'world> = DataView;
    type Item<'world> = Option<&'world T>;

    // Due to `Option`, this data will not affect the filter.
    const COMPONENTS_ARE_DENSE: bool = false;

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn try_build_state(world: &World) -> Option<Self::State> {
        world.get_component_id::<T>()
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
// Ref

unsafe impl<T: Component> ReadOnlyQueryData for Ref<'_, T> {}

unsafe impl<T: Component> QueryData for Ref<'_, T> {
    type ReadOnly = Self;
    type State = ComponentId;
    type Cache<'world> = ComponentView;
    type Item<'world> = Ref<'world, T>;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn try_build_state(world: &World) -> Option<Self::State> {
        world.get_component_id::<T>()
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        match T::STORAGE {
            StorageMode::Dense => ComponentView::build_dense(last_run, this_run),
            StorageMode::Sparse => ComponentView::build_sparse(*state, world, last_run, this_run),
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
        let last_run = cache.last_run;
        let this_run = cache.this_run;
        match T::STORAGE {
            StorageMode::Dense => {
                let ptr = unsafe { cache.data.dense }?;
                let column = unsafe { &*ptr.as_ptr() };
                let row = table_row.0 as usize;
                let untyped = unsafe { column.get_ref(row, last_run, this_run) };
                unsafe { Some(untyped.with_type::<T>()) }
            }
            StorageMode::Sparse => {
                let ptr = unsafe { cache.data.sparse }?;
                let map = unsafe { &*ptr.as_ptr() };
                let row = map.get_map_row(entity)?;
                let untyped = unsafe { map.get_ref(row, last_run, this_run) };
                unsafe { Some(untyped.with_type::<T>()) }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Option<Ref<'_, T>>

unsafe impl<T: Component> ReadOnlyQueryData for Option<Ref<'_, T>> {}

unsafe impl<T: Component> QueryData for Option<Ref<'_, T>> {
    type ReadOnly = Self;
    type State = ComponentId;
    type Cache<'world> = ComponentView;
    type Item<'world> = Option<Ref<'world, T>>;

    // Due to `Option`, this data will not affect the filter.
    const COMPONENTS_ARE_DENSE: bool = false;

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn try_build_state(world: &World) -> Option<Self::State> {
        world.get_component_id::<T>()
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        unsafe { <Ref<T> as QueryData>::build_cache(state, world, last_run, this_run) }
    }

    fn build_filter(_state: &Self::State, _out: &mut Vec<FilterParamBuilder>) {
        // Because `Option`, we do not set filter.
    }

    fn build_access(state: &Self::State, out: &mut AccessParam) -> bool {
        <Ref<T> as QueryData>::build_access(state, out)
    }

    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        arche: &'w Archetype,
        table: &'w Table,
    ) {
        unsafe { <Ref<T> as QueryData>::set_for_arche(state, cache, arche, table) }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        unsafe { <Ref<T> as QueryData>::set_for_table(state, cache, table) }
    }

    unsafe fn fetch<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w>> {
        Some(unsafe { <Ref<T> as QueryData>::fetch(state, cache, entity, table_row) })
    }
}

// -----------------------------------------------------------------------------
// &mut T

unsafe impl<T: Component> QueryData for &mut T {
    // Downgrade to Ref<T> to preserve change-tick metadata in read-only mode.
    type ReadOnly = Ref<'static, T>;
    type State = ComponentId;
    type Cache<'world> = ComponentView;
    type Item<'world> = Mut<'world, T>;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn try_build_state(world: &World) -> Option<Self::State> {
        world.get_component_id::<T>()
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        match T::STORAGE {
            StorageMode::Dense => ComponentView::build_dense(last_run, this_run),
            StorageMode::Sparse => ComponentView::build_sparse(*state, world, last_run, this_run),
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
        let last_run = cache.last_run;
        let this_run = cache.this_run;
        match T::STORAGE {
            StorageMode::Dense => {
                let ptr = unsafe { cache.data.dense }?;
                let column = unsafe { &mut *ptr.as_ptr() };
                let row = table_row.0 as usize;
                let untyped = unsafe { column.get_mut(row, last_run, this_run) };
                unsafe { Some(untyped.with_type::<T>()) }
            }
            StorageMode::Sparse => {
                let ptr = unsafe { cache.data.sparse }?;
                let map = unsafe { &mut *ptr.as_ptr() };
                let row = map.get_map_row(entity)?;
                let untyped = unsafe { map.get_mut(row, last_run, this_run) };
                unsafe { Some(untyped.with_type::<T>()) }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Option<&mut T>

unsafe impl<T: Component> QueryData for Option<&mut T> {
    type ReadOnly = Option<Ref<'static, T>>;
    type State = ComponentId;
    type Cache<'world> = ComponentView;
    type Item<'world> = Option<Mut<'world, T>>;

    // Due to `Option`, this data will not affect the filter.
    const COMPONENTS_ARE_DENSE: bool = false;

    fn build_state(world: &mut World) -> Self::State {
        world.register_component::<T>()
    }

    fn try_build_state(world: &World) -> Option<Self::State> {
        world.get_component_id::<T>()
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
        unsafe { <&mut T as QueryData>::set_for_arche(state, cache, arche, table) }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        unsafe { <&mut T as QueryData>::set_for_table(state, cache, table) }
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

// -----------------------------------------------------------------------------
// Mut

unsafe impl<T: Component> QueryData for Mut<'_, T> {
    type ReadOnly = Ref<'static, T>;
    type State = ComponentId;
    type Cache<'world> = ComponentView;
    type Item<'world> = Mut<'world, T>;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();

    fn build_state(world: &mut World) -> Self::State {
        <&mut T as QueryData>::build_state(world)
    }

    fn try_build_state(world: &World) -> Option<Self::State> {
        <&mut T as QueryData>::try_build_state(world)
    }

    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        unsafe { <&mut T as QueryData>::build_cache(state, world, last_run, this_run) }
    }

    fn build_filter(state: &Self::State, out: &mut Vec<FilterParamBuilder>) {
        <&mut T as QueryData>::build_filter(state, out);
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
        unsafe { <&mut T as QueryData>::fetch(state, cache, entity, table_row) }
    }
}

// -----------------------------------------------------------------------------
// Option<Mut<'_, T>>

unsafe impl<T: Component> QueryData for Option<Mut<'_, T>> {
    type ReadOnly = Option<Ref<'static, T>>;
    type State = ComponentId;
    type Cache<'world> = ComponentView;
    type Item<'world> = Option<Mut<'world, T>>;

    // Due to `Option`, this data will not affect the filter.
    const COMPONENTS_ARE_DENSE: bool = false;

    fn build_state(world: &mut World) -> Self::State {
        <&mut T as QueryData>::build_state(world)
    }

    fn try_build_state(world: &World) -> Option<Self::State> {
        <&mut T as QueryData>::try_build_state(world)
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
        unsafe { <&mut T as QueryData>::set_for_arche(state, cache, arche, table) }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        unsafe { <&mut T as QueryData>::set_for_table(state, cache, table) }
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
