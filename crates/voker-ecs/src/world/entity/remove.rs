use crate::archetype::ArcheId;
use crate::bundle::Bundle;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::EntityOwned;

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
    /// For the specified set of components to remove `A`:
    /// 1. Attempt to remove `A`'s required components `B`
    /// 2. Only remove the removable parts, i.e., components that exist and are not
    ///    depended upon by components outside of `A + B`.
    ///
    /// For example, given an entity `(A, B, C)` where `B` requires `A`:
    /// You cannot remove only `A`. When removing `B`, `A` will be automatically removed.
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
    /// assert!(entity.contains::<Bar>());
    ///
    /// entity.remove::<Bar>();
    /// assert!(!entity.contains::<Bar>());
    /// ```
    #[track_caller]
    pub fn remove<B: Bundle>(&mut self) {
        let world = unsafe { self.world.full_mut() };
        let bundle_id = world.register_bundle::<B>();
        let old_arche_id = self.location.arche_id;
        let new_arche_id = world.arche_after_remove(old_arche_id, bundle_id);

        let guard = ForgetEntityOnPanic {
            entity: self.entity,
            world: self.world,
            location: DebugLocation::caller(),
        };

        if old_arche_id != new_arche_id {
            self.remove_moved(new_arche_id);
        }

        ::core::mem::forget(guard);
    }

    #[inline(never)]
    fn remove_moved(&mut self, new_arche_id: ArcheId) {
        let old_arche_id = self.location.arche_id;
        let old_arche = unsafe { self.world.full_mut().archetypes.get_unchecked_mut(old_arche_id) };
        let new_arche = unsafe { self.world.full_mut().archetypes.get_unchecked_mut(new_arche_id) };
        assert_eq!(old_arche.table_id(), self.location.table_id);

        let moved = unsafe { old_arche.remove_entity(self.location.arche_row) };
        unsafe {
            self.world.full_mut().entities.update_row(moved).unwrap();
        }
        let new_arche_row = unsafe { new_arche.insert_entity(self.entity) };
        self.location.arche_id = new_arche_id;
        self.location.arche_row = new_arche_row;

        let old_table_id = old_arche.table_id();
        let new_table_id = new_arche.table_id();

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
    }
}
