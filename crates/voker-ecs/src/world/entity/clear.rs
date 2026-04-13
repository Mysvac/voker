use crate::archetype::ArcheId;
use crate::storage::TableId;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned};

impl EntityOwned<'_> {
    /// Removes all components associated with the entity.
    ///
    /// # Panics
    ///
    /// Panics if this entity is despawned.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_ecs::prelude::*;
    ///
    /// #[derive(Default, Component, Clone)]
    /// struct Foo;
    ///
    /// #[derive(Component, Clone)]
    /// #[component(required = Foo)]
    /// struct Bar;
    ///
    /// let mut world = World::alloc();
    ///
    /// let mut entity = world.spawn(Bar);
    /// assert!(entity.contains::<Foo>());
    /// assert!(entity.contains::<Bar>());
    ///
    /// entity.clear();
    /// assert!(!entity.contains::<Foo>()); // removed
    /// assert!(!entity.contains::<Bar>());
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn clear(&mut self) -> &mut Self {
        self.clear_with_caller(DebugLocation::caller());
        self
    }

    pub(crate) fn clear_with_caller(&mut self, caller: DebugLocation) {
        self.assert_is_spawned_with_caller(caller);

        let mut location = unsafe { self.location.take().unwrap_unchecked() };
        let unsafe_world = self.world;
        let entity = self.entity;

        if location.arche_id == ArcheId::EMPTY {
            self.location = Some(location);
            return;
        }

        // Create guard after Empty checking.
        let guard = ForgetEntityOnPanic {
            entity,
            world: unsafe_world,
            caller,
        };

        let old_arche_id = location.arche_id;
        let new_arche_id = ArcheId::EMPTY;
        let [old_arche, new_arche] = unsafe {
            let arches = &mut unsafe_world.full_mut().archetypes;
            let indices = [old_arche_id.index(), new_arche_id.index()];
            arches.as_mut_slice().get_disjoint_unchecked_mut(indices)
        };
        debug_assert_eq!(old_arche.table_id(), location.table_id);

        {
            // trigger component hooks
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            old_arche.trigger_on_discard(entity, world.reborrow(), caller);
            old_arche.trigger_on_remove(entity, world.reborrow(), caller);
        }

        {
            // update row
            let new_arche_row = unsafe {
                let moved = old_arche.dealloc_row(location.arche_row);
                self.world.full_mut().entities.update_row(moved).unwrap();
                new_arche.alloc_row(self.entity)
            };
            location.arche_id = new_arche_id;
            location.arche_row = new_arche_row;
        }

        {
            // update table
            let old_table_id = old_arche.table_id();
            let new_table_id = TableId::EMPTY;

            if old_table_id != new_table_id {
                let table_row = location.table_row;
                let [old_table, new_table] = unsafe {
                    let tables = &mut unsafe_world.full_mut().storages.tables;
                    let indices = [old_table_id.index(), new_table_id.index()];
                    tables.as_mut_slice().get_disjoint_unchecked_mut(indices)
                };
                let new_row = unsafe {
                    let (moved, new) = old_table.move_row::<true>(table_row, new_table);
                    unsafe_world.full_mut().entities.update_row(moved).unwrap();
                    new
                };
                location.table_id = new_table_id;
                location.table_row = new_row;
            }
        }

        {
            // clear map date
            let world = unsafe { self.world.full_mut() };
            let maps = &mut world.storages.maps;
            old_arche.sparse_components().iter().for_each(|&id| {
                let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
                let map = unsafe { maps.get_unchecked_mut(map_id) };
                let map_row = map.get_map_row(entity).unwrap();
                unsafe { map.dealloc_row::<true>(map_row) };
            });
        }

        unsafe {
            let world = self.world.full_mut();
            world.entities.update_location(entity, location).unwrap();
            world.flush();
        }

        ::core::mem::forget(guard);

        self.relocate();
    }
}
