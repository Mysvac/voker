use crate::archetype::ArcheId;
use crate::bundle::Bundle;
use crate::component::ComponentHookContext;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned};

impl EntityOwned<'_> {
    /// Remove component.
    ///
    /// # Rules
    ///
    /// ## If some components do not exist
    ///
    /// Only existing components are removed; the program runs normally.
    ///
    /// ## If required components are involved
    ///
    /// Only remove the removable parts.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::world::World;
    /// # use voker_ecs::component::Component;
    /// # #[derive(Component, Debug)]
    /// # struct Foo;
    /// # #[derive(Component, Debug)]
    /// # struct Bar;
    /// let mut world = World::alloc();
    ///
    /// let mut entity = world.spawn((Foo, Bar));
    /// assert!(entity.contains::<Foo>());
    /// assert!(entity.contains::<Bar>());
    ///
    /// entity.remove::<Bar>();
    /// assert!(entity.contains::<Foo>());
    /// assert!(!entity.contains::<Bar>());
    /// ```
    #[inline]
    #[track_caller]
    pub fn remove<B: Bundle>(&mut self) {
        self.remove_with_caller::<B>(DebugLocation::caller());
    }

    pub(crate) fn remove_with_caller<B: Bundle>(&mut self, caller: DebugLocation) {
        let world = unsafe { self.world.full_mut() };
        let bundle_id = world.register_explicit_bundle::<B>();
        let old_arche_id = self.location.arche_id;
        let new_arche_id = world.arche_after_remove(old_arche_id, bundle_id);

        let guard = ForgetEntityOnPanic {
            entity: self.entity,
            world: self.world,
            location: caller,
        };

        if old_arche_id != new_arche_id {
            self.remove_moved(new_arche_id, caller);
        }

        ::core::mem::forget(guard);
    }

    #[inline(never)]
    fn remove_moved(&mut self, new_arche_id: ArcheId, caller: DebugLocation) {
        let unsafe_world = self.world;
        let entity = self.entity;

        let old_arche_id = self.location.arche_id;
        let old_arche =
            unsafe { unsafe_world.full_mut().archetypes.get_unchecked_mut(old_arche_id) };
        let new_arche =
            unsafe { unsafe_world.full_mut().archetypes.get_unchecked_mut(new_arche_id) };
        debug_assert_eq!(old_arche.table_id(), self.location.table_id);

        {
            // trigger_on_discard
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            old_arche.discard_hooks().iter().for_each(|&(id, hook)| {
                if !new_arche.contains_component(id) {
                    hook(
                        world.reborrow(),
                        ComponentHookContext { id, entity, caller },
                    );
                }
            });
        }
        {
            // trigger_on_remove
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            old_arche.remove_hooks().iter().for_each(|&(id, hook)| {
                if !new_arche.contains_component(id) {
                    hook(
                        world.reborrow(),
                        ComponentHookContext { id, entity, caller },
                    );
                }
            });
        }

        // Move Arche
        let new_arche_row = unsafe {
            let moved = old_arche.remove_entity(self.location.arche_row);
            self.world.full_mut().entities.update_row(moved).unwrap();
            new_arche.insert_entity(self.entity)
        };

        self.location.arche_id = new_arche_id;
        self.location.arche_row = new_arche_row;

        // Move Table
        let old_table_id = old_arche.table_id();
        let new_table_id = new_arche.table_id();

        // Move Table
        if old_table_id != new_table_id {
            let table_row = self.location.table_row;
            let old_table =
                unsafe { self.world.data_mut().storages.tables.get_unchecked_mut(old_table_id) };
            let new_table =
                unsafe { self.world.data_mut().storages.tables.get_unchecked_mut(new_table_id) };
            let (moved, new_row) =
                unsafe { old_table.move_to_and_drop_missing(table_row, new_table) };
            unsafe {
                self.world.full_mut().entities.update_row(moved).unwrap();
            }
            self.location.table_id = new_table_id;
            self.location.table_row = new_row;
        }

        // Move Map
        let world = unsafe { self.world.full_mut() };
        let maps = &mut world.storages.maps;
        old_arche.sparse_components().iter().for_each(|&id| {
            if !new_arche.contains_sparse_component(id) {
                let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
                let map = unsafe { maps.get_unchecked_mut(map_id) };
                let row = unsafe { map.deallocate(self.entity).unwrap() };
                unsafe {
                    map.drop_item(row);
                }
            }
        });

        unsafe {
            world.entities.update_location(self.entity, self.location).unwrap();
        }

        world.flush();
    }
}
