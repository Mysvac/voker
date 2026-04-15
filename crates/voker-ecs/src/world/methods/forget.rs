use crate::entity::Entity;
use crate::utils::DebugLocation;
use crate::world::World;

impl World {
    /// Forget an entity without dropping its data and without calling components' hooks.
    ///
    /// Typically used for cleaning up entities that caused a panic.
    ///
    /// # Safety
    /// This operation is **extremely unsafe** and should be used with extreme caution.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub unsafe fn forget(&mut self, entity: Entity) {
        unsafe {
            self.forget_with_caller(entity, DebugLocation::caller());
        }
    }

    #[inline]
    pub(crate) unsafe fn forget_with_caller(&mut self, entity: Entity, caller: DebugLocation) {
        let world_id = self.id();

        log::warn!(
            "Entity<{entity}>(in World<{world_id}>) was forgotten, may leaking memory: {caller}."
        );

        let _ = unsafe { self.entities.set_despawned(entity) };
        for arche in self.archetypes.iter_mut() {
            if let Some(row) = arche.get_arche_row(entity) {
                let moved = unsafe { arche.dealloc_row(row) };
                unsafe {
                    self.entities.update_row(moved).unwrap();
                }
            }
        }

        for table in self.storages.tables.iter_mut() {
            if let Some(row) = table.get_table_row(entity) {
                let moved = unsafe { table.dealloc_row::<false>(row) };
                unsafe {
                    self.entities.update_row(moved).unwrap();
                }
            }
        }

        for map in self.storages.maps.iter_mut() {
            if let Some(row) = map.get_map_row(entity) {
                unsafe { map.dealloc_row::<false>(row) };
            }
        }
    }
}
