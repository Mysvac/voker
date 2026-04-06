use crate::archetype::ArcheId;
use crate::storage::TableId;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned};

impl EntityOwned<'_> {
    #[inline]
    #[track_caller]
    pub fn clear(&mut self) {
        self.clear_with_caller(DebugLocation::caller());
    }

    pub(crate) fn clear_with_caller(&mut self, caller: DebugLocation) {
        let location = self.location;
        let unsafe_world = self.world;
        let entity = self.entity;

        if location.arche_id == ArcheId::EMPTY {
            return;
        }

        let guard = ForgetEntityOnPanic {
            entity,
            world: unsafe_world,
            location: caller,
        };

        let old_arche_id = self.location.arche_id;
        let new_arche_id = ArcheId::EMPTY;
        let old_arche =
            unsafe { unsafe_world.full_mut().archetypes.get_unchecked_mut(old_arche_id) };
        let new_arche =
            unsafe { unsafe_world.full_mut().archetypes.get_unchecked_mut(new_arche_id) };
        debug_assert_eq!(old_arche.table_id(), self.location.table_id);

        {
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            old_arche.trigger_on_discard(entity, world.reborrow());
            old_arche.trigger_on_remove(entity, world.reborrow());
        }

        let new_arche_row = unsafe {
            let moved = old_arche.remove_entity(self.location.arche_row);
            self.world.full_mut().entities.update_row(moved).unwrap();
            new_arche.insert_entity(self.entity)
        };
        self.location.arche_id = new_arche_id;
        self.location.arche_row = new_arche_row;

        // Move Table
        let old_table_id = old_arche.table_id();
        let new_table_id = TableId::EMPTY;
        debug_assert_eq!(new_arche.table_id(), TableId::EMPTY);

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
            let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
            let map = unsafe { maps.get_unchecked_mut(map_id) };
            let row = unsafe { map.deallocate(entity).unwrap() };
            unsafe {
                map.drop_item(row);
            }
        });

        unsafe {
            world.entities.update_location(entity, self.location).unwrap();
        }

        ::core::mem::forget(guard);

        world.flush();
    }
}
