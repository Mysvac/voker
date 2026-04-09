use crate::entity::Entity;
use crate::link::LinkHookMode;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned};

impl EntityOwned<'_> {
    /// Despawns the current entity.
    ///
    /// If `self.location` is none, this function is no-op.
    #[inline]
    #[track_caller]
    pub fn despawn(self) {
        self.despawn_with_caller(DebugLocation::caller());
    }

    /// Despawns the entity without freeing it to the allocator.
    ///
    /// If `self.location` is none, return `self.entity` directly.
    ///
    /// This returns the new [`Entity`], which you must manage. Note that this still
    /// increases the generation(tag) to differentiate different spawns of the same row.
    ///
    /// Additionally, keep in mind the limitations documented in the type-level docs.
    /// Unless you have full knowledge of this [`EntityOwned`]'s lifetime, you may not
    /// assume that nothing else has taken responsibility of this [`Entity`]. If you are
    /// not careful, this could cause a double free.
    #[inline]
    #[track_caller]
    pub fn despawn_no_free(mut self) -> Entity {
        self.despawn_no_free_with_caller(DebugLocation::caller());
        self.entity
    }

    #[inline]
    pub(crate) fn despawn_with_caller(mut self, caller: DebugLocation) {
        self.despawn_no_free_with_caller(caller);
        let world = unsafe { self.world.data_mut() };

        // Ok(None) -> Tag matched, but the entity is despawned.
        if let Ok(None) = world.entities.try_locate(self.entity) {
            world.allocator.free(self.entity);
        }
    }

    pub(crate) fn despawn_no_free_with_caller(&mut self, caller: DebugLocation) {
        let Some(location) = self.location.take() else {
            return;
        };

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
            // Trigger component hooks
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            let link_hook_mode = LinkHookMode::Run;
            arche.trigger_on_despawn(entity, world.reborrow(), link_hook_mode, caller);
            arche.trigger_on_discard(entity, world.reborrow(), link_hook_mode, caller);
            arche.trigger_on_remove(entity, world.reborrow(), link_hook_mode, caller);
        }

        let move_res1 = {
            // move archetype
            let arche_moved = unsafe { arche.remove_entity(arche_row) };
            unsafe { world.entities.update_row(arche_moved) }
        };

        let move_res2 = {
            // move table data
            let table_id = location.table_id;
            let table_row = location.table_row;
            let table = unsafe { world.storages.tables.get_unchecked_mut(table_id) };
            let table_moved = unsafe { table.swap_remove::<true>(table_row) };
            unsafe { world.entities.update_row(table_moved) }
        };

        {
            // move map data
            let maps = &mut world.storages.maps;
            arche.sparse_components().iter().for_each(|&cid| unsafe {
                let map_id = maps.get_id(cid).debug_checked_unwrap();
                let map = maps.get_unchecked_mut(map_id);
                let map_row = map.deallocate(entity).unwrap();
                map.drop_item(map_row);
            });
        }

        ::core::mem::forget(guard);

        move_res1.unwrap_or_else(|e| panic!("{e} {caller}"));
        move_res2.unwrap_or_else(|e| panic!("{e} {caller}"));

        // Free before flush.
        self.entity = unsafe { world.entities.free(entity.id(), 1) };

        world.flush();
    }
}
