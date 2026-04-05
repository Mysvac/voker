use crate::entity::Entity;
use crate::utils::DebugLocation;
use crate::world::UnsafeWorld;

pub struct ForgetEntityOnPanic<'a> {
    pub entity: Entity,
    pub world: UnsafeWorld<'a>,
    pub location: DebugLocation,
}

impl Drop for ForgetEntityOnPanic<'_> {
    #[cold]
    #[inline(never)]
    fn drop(&mut self) {
        let world = unsafe { self.world.full_mut() };
        let world_id = world.id();
        let entity = self.entity;
        let location = self.location;

        log::error!(
            "Entity<{entity}>(in World<{world_id}>) panicked, may leaking memory: {location}."
        );

        let _ = unsafe { world.entities.set_despawned(entity) };
        for arche in world.archetypes.iter_mut() {
            if let Some(row) = arche.get_arche_row(entity) {
                let moved = unsafe { arche.remove_entity(row) };
                unsafe {
                    world.entities.update_row(moved).unwrap();
                }
            }
        }

        for table in world.storages.tables.iter_mut() {
            if let Some(row) = table.get_table_row(entity) {
                let moved = unsafe { table.swap_remove_and_forget(row) };
                unsafe {
                    world.entities.update_row(moved).unwrap();
                }
            }
        }

        for map in world.storages.maps.iter_mut() {
            let _ = unsafe { map.deallocate(entity) };
        }
    }
}
