use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned};

impl EntityOwned<'_> {
    #[inline]
    #[track_caller]
    pub fn despawn(self) {
        self.despawn_with_caller(DebugLocation::caller());
    }

    pub(crate) fn despawn_with_caller(self, caller: DebugLocation) {
        let location = self.location;
        let unsafe_world = self.world;
        let entity = self.entity;

        let guard = ForgetEntityOnPanic {
            entity,
            world: unsafe_world,
            location: caller,
        };

        let world = unsafe { unsafe_world.full_mut() };

        let arche_id = location.arche_id;
        let arche_row = location.arche_row;
        let arche = unsafe { world.archetypes.get_unchecked_mut(arche_id) };

        {
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            arche.trigger_on_despawn(entity, world.reborrow());
            arche.trigger_on_discard(entity, world.reborrow());
            arche.trigger_on_remove(entity, world.reborrow());
        }

        let arche_moved = unsafe { arche.remove_entity(arche_row) };
        let move_res1 = unsafe { world.entities.update_row(arche_moved) };

        let table_id = location.table_id;
        let table_row = location.table_row;
        let table = unsafe { world.storages.tables.get_unchecked_mut(table_id) };
        let table_moved = unsafe { table.swap_remove_and_drop(table_row) };
        let move_res2 = unsafe { world.entities.update_row(table_moved) };

        let maps = &mut world.storages.maps;
        arche.sparse_components().iter().for_each(|&cid| unsafe {
            let map_id = maps.get_id(cid).debug_checked_unwrap();
            let map = maps.get_unchecked_mut(map_id);
            let map_row = map.deallocate(entity).unwrap();
            map.drop_item(map_row);
        });

        ::core::mem::forget(guard);

        world.flush();

        move_res1.unwrap();
        move_res2.unwrap();

        let new_entity = unsafe { world.entities.free(entity.id(), 1) };
        world.allocator.free(new_entity);
    }
}
