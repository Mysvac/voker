use crate::archetype::ArcheId;
use crate::entity::EntityLocation;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{EntityMut, World};

impl World {
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub unsafe fn spawn_uninit(&mut self, arche_id: ArcheId) -> EntityMut<'_> {
        let caller = DebugLocation::caller();
        unsafe { self.spawn_uninit_with_caller(arche_id, caller) }
    }

    pub unsafe fn spawn_uninit_with_caller(
        &mut self,
        arche_id: ArcheId,
        caller: DebugLocation,
    ) -> EntityMut<'_> {
        let entity = self.allocator.alloc_mut();

        if ::core::cfg!(debug_assertions) {
            self.entities.can_spawn(entity).unwrap();
        }

        let unsafe_world = self.unsafe_world();

        let guard = ForgetEntityOnPanic {
            entity,
            world: unsafe_world,
            caller,
        };

        let world = unsafe { unsafe_world.full_mut() };

        let arche = unsafe { world.archetypes.get_unchecked_mut(arche_id) };
        let table_id = arche.table_id();
        let table = unsafe { world.storages.tables.get_unchecked_mut(table_id) };
        let maps = &mut world.storages.maps;

        let arche_row = unsafe { arche.alloc_row(entity) };
        let table_row = unsafe { table.alloc_row(entity) };
        arche.sparse_components().iter().for_each(|&cid| unsafe {
            let map_id = maps.get_id(cid).debug_checked_unwrap();
            let map = maps.get_unchecked_mut(map_id);
            let _ = map.alloc_row(entity); // `MapRow` may be cached in the future.
        });

        let location = EntityLocation {
            arche_id,
            arche_row,
            table_id,
            table_row,
        };

        unsafe {
            world.entities.set_spawned(entity, location).unwrap();
        }

        ::core::mem::forget(guard);

        let this_run = self.this_run_fast();
        let last_run = self.last_run();
        let world = self.unsafe_world();

        EntityMut {
            last_run,
            this_run,
            world,
            entity,
            location,
        }
    }
}
