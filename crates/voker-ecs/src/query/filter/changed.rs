use alloc::vec::Vec;
use voker_ptr::ThinSlice;

use super::QueryFilter;
use crate::archetype::Archetype;
use crate::component::{Component, ComponentId, StorageMode};
use crate::entity::Entity;
use crate::storage::{Map, Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// Changed

/// Query filter that matches entities whose component `T` changed
/// in the current system run interval.
///
/// This checks whether the component's changed tick is newer than
/// `(last_run, this_run]`.
///
/// Notes:
/// - The filter only matches entities that currently contain `T`.
/// - It performs entity-level filtering during iteration.
/// - It supports both dense and sparse component storage.
///
/// # Examples
///
/// ```no_run
/// use voker_ecs::prelude::*;
///
/// #[derive(Component, Clone)]
/// struct Velocity(f32);
///
/// fn changed_velocity(query: Query<Entity, Changed<Velocity>>) {
///     for _entity in query {
///         // Entities where `Velocity` changed since last run.
///     }
/// }
/// ```
pub struct Changed<T: Component>(T);

// -----------------------------------------------------------------------------
// QueryFilter Implementation

union StorageSwitch<'w> {
    dense: Option<ThinSlice<'w, Tick>>,
    sparse: Option<&'w Map>,
}

pub struct ChangedView<'w> {
    ticks: StorageSwitch<'w>,
    last_run: Tick,
    this_run: Tick,
}

unsafe impl<T: Component> QueryFilter for Changed<T> {
    type State = ComponentId;
    type Cache<'world> = ChangedView<'world>;

    const COMPONENTS_ARE_DENSE: bool = T::STORAGE.is_dense();
    const ENABLE_ENTITY_FILTER: bool = true;

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
            StorageMode::Dense => ChangedView {
                ticks: StorageSwitch { dense: None },
                last_run,
                this_run,
            },
            StorageMode::Sparse => {
                let maps = unsafe { &world.read_only().storages.maps };
                if let Some(map_id) = maps.get_id(*state) {
                    ChangedView {
                        ticks: StorageSwitch {
                            sparse: maps.get(map_id),
                        },
                        last_run,
                        this_run,
                    }
                } else {
                    ChangedView {
                        ticks: StorageSwitch { sparse: None },
                        last_run,
                        this_run,
                    }
                }
            }
        }
    }

    fn build_filter(state: &Self::State, outer: &mut Vec<FilterParamBuilder>) {
        let mut builder = FilterParamBuilder::new();
        builder.with(*state);
        outer.push(builder);
    }

    fn build_access(state: &Self::State, out: &mut AccessParam) {
        // We need to access tick data. But since the filter
        // occurs before the visitor, there will be no conflict.
        out.force_reading(*state);
    }

    fn edit_access_table(_state: &Self::State, _table: &mut crate::system::AccessTable) -> bool {
        true // We don't need to access resources
    }

    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        _arche: &'w Archetype,
        table: &'w Table,
    ) {
        if T::STORAGE.is_dense() {
            let Some(table_col) = table.get_table_col(*state) else {
                cache.ticks = StorageSwitch { dense: None };
                return;
            };
            let slice = unsafe { ThinSlice::from_ref(table.get_changed_slice(table_col)) };
            cache.ticks = StorageSwitch { dense: Some(slice) };
        }
    }

    unsafe fn set_for_table<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        table: &'w Table,
    ) {
        if T::STORAGE.is_dense() {
            let Some(table_col) = table.get_table_col(*state) else {
                cache.ticks = StorageSwitch { dense: None };
                return;
            };
            let slice = unsafe { ThinSlice::from_ref(table.get_changed_slice(table_col)) };
            cache.ticks = StorageSwitch { dense: Some(slice) };
        }
    }

    unsafe fn filter<'w>(
        _state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool {
        match T::STORAGE {
            StorageMode::Dense => {
                let dense = unsafe { cache.ticks.dense };
                let Some(slice) = dense else {
                    return false;
                };
                let changed = unsafe { *slice.get(table_row.0 as usize) };
                changed.is_newer_than(cache.last_run, cache.this_run)
            }
            StorageMode::Sparse => {
                let sparse = unsafe { cache.ticks.sparse };
                let Some(map) = sparse else {
                    return false;
                };
                let Some(map_row) = map.get_map_row(entity) else {
                    return false;
                };
                let changed = unsafe { map.get_changed(map_row) };
                changed.is_newer_than(cache.last_run, cache.this_run)
            }
        }
    }
}
