use alloc::vec::Vec;

use super::{QueryData, ReadOnlyQueryData};
use crate::archetype::Archetype;
use crate::entity::Entity;
use crate::storage::{Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{EntityMut, EntityRef, UnsafeWorld, World};

// -----------------------------------------------------------------------------
// Entity

unsafe impl ReadOnlyQueryData for Entity {}

unsafe impl QueryData for Entity {
    type ReadOnly = Self;
    type State = ();
    type Cache<'world> = ();
    type Item<'world> = Entity;

    const COMPONENTS_ARE_DENSE: bool = true;

    fn build_state(_world: &mut World) -> Self::State {}

    fn try_build_state(_world: &World) -> Option<Self::State> {
        Some(())
    }

    unsafe fn build_cache<'w>(
        _state: &Self::State,
        _world: UnsafeWorld<'w>,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Cache<'w> {
    }

    fn build_filter(_state: &Self::State, _out: &mut Vec<FilterParamBuilder>) {}

    fn build_access(_state: &Self::State, _out: &mut AccessParam) -> bool {
        true // We did not access any components
    }

    unsafe fn set_for_arche<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _arche: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    unsafe fn set_for_table<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _table: &'w Table,
    ) {
    }

    unsafe fn fetch<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w>> {
        Some(entity)
    }
}

// -----------------------------------------------------------------------------
// EntityRef & EntityMut

pub struct EntityView<'w> {
    world: UnsafeWorld<'w>,
    last_run: Tick,
    this_run: Tick,
}

unsafe impl ReadOnlyQueryData for EntityRef<'_> {}

unsafe impl QueryData for EntityRef<'_> {
    type ReadOnly = Self;
    type State = ();
    type Cache<'world> = EntityView<'world>;
    type Item<'world> = EntityRef<'world>;

    const COMPONENTS_ARE_DENSE: bool = true;

    fn build_state(_world: &mut World) -> Self::State {}

    fn try_build_state(_world: &World) -> Option<Self::State> {
        Some(())
    }

    unsafe fn build_cache<'w>(
        _state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        EntityView {
            world,
            last_run,
            this_run,
        }
    }

    fn build_filter(_state: &Self::State, _out: &mut Vec<FilterParamBuilder>) {}

    fn build_access(_state: &Self::State, out: &mut AccessParam) -> bool {
        out.set_entity_ref()
    }

    unsafe fn set_for_arche<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _arche: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    unsafe fn set_for_table<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _table: &'w Table,
    ) {
    }

    unsafe fn fetch<'w>(
        _state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w>> {
        let world = unsafe { cache.world.read_only() };
        let location = world.entities.locate(entity).unwrap();
        Some(EntityRef {
            world: world.unsafe_world(),
            entity,
            location,
            last_run: cache.last_run,
            this_run: cache.this_run,
        })
    }
}

unsafe impl QueryData for EntityMut<'_> {
    type ReadOnly = EntityRef<'static>;
    type State = ();
    type Cache<'world> = EntityView<'world>;
    type Item<'world> = EntityMut<'world>;

    const COMPONENTS_ARE_DENSE: bool = true;

    fn build_state(_world: &mut World) -> Self::State {}

    fn try_build_state(_world: &World) -> Option<Self::State> {
        Some(())
    }

    unsafe fn build_cache<'w>(
        _state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w> {
        EntityView {
            world,
            last_run,
            this_run,
        }
    }

    fn build_filter(_state: &Self::State, _out: &mut Vec<FilterParamBuilder>) {}

    fn build_access(_state: &Self::State, out: &mut AccessParam) -> bool {
        out.set_entity_mut()
    }

    unsafe fn set_for_arche<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _arche: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    unsafe fn set_for_table<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _table: &'w Table,
    ) {
    }

    unsafe fn fetch<'w>(
        _state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        _table_row: TableRow,
    ) -> Option<Self::Item<'w>> {
        let world = unsafe { cache.world.data_mut() };
        let location = world.entities.locate(entity).unwrap();
        Some(EntityMut {
            world: world.unsafe_world(),
            entity,
            location,
            last_run: cache.last_run,
            this_run: cache.this_run,
        })
    }
}
