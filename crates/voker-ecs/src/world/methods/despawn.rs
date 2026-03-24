use crate::entity::{Entity, EntityError};
use crate::utils::DebugCheckedUnwrap;
use crate::world::World;

impl World {
    /// Despawns an entity and removes all of its components.
    ///
    /// This operation:
    /// - Marks the entity as despawned in the entity registry.
    /// - Removes the entity row from its archetype and table.
    /// - Drops sparse-component values associated with that entity.
    /// - Fixes moved-entity locations caused by swap-remove operations.
    /// - Releases the entity id back to the allocator.
    ///
    /// # Errors
    ///
    /// Returns [`EntityError`] if the entity is invalid or is not currently
    /// spawned in this world.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::component::Component;
    /// # use voker_ecs::world::World;
    /// # #[derive(Component, Debug)]
    /// # struct Foo;
    /// #
    /// # let mut world = World::default();
    /// let entity = world.spawn(Foo).entity();
    /// assert!(world.despawn(entity).is_ok());
    ///
    /// // Despawning the same entity again returns an error.
    /// assert!(world.despawn(entity).is_err());
    /// ```
    pub fn despawn(&mut self, entity: Entity) -> Result<(), EntityError> {
        let location = unsafe { self.entities.set_despawned(entity)? };

        let arche_id = location.arche_id;
        let arche_row = location.arche_row;
        let archetype = unsafe { self.archetypes.get_unchecked_mut(arche_id) };
        let arche_moved = unsafe { archetype.remove_entity(arche_row) };

        let table_id = location.table_id;
        let table_row = location.table_row;
        let table = unsafe { self.storages.tables.get_unchecked_mut(table_id) };
        let table_moved = unsafe { table.swap_remove_and_drop(table_row) };

        let maps = &mut self.storages.maps;
        archetype
            .sparse_components()
            .iter()
            .for_each(|&cid| unsafe {
                let map_id = maps.get_id(cid).debug_checked_unwrap();
                let map = maps.get_unchecked_mut(map_id);
                let map_row = map.deallocate(entity).debug_checked_unwrap();
                map.drop_item(map_row);
            });

        let new_entity = unsafe { self.entities.free(entity.id(), 1) };
        self.allocator.free(new_entity);

        let res1 = unsafe { self.entities.update_row(arche_moved) };
        let res2 = unsafe { self.entities.update_row(table_moved) };
        res1.and(res2)
    }
}

#[cfg(test)]
mod tests {
    use crate::component::{Component, ComponentStorage};
    use crate::world::{World, WorldIdAllocator};
    use alloc::string::String;
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, PartialEq, Eq)]
    struct Foo;

    #[derive(Debug, PartialEq, Eq)]
    struct Bar(u64);

    #[derive(Debug, PartialEq, Eq)]
    struct Baz(String);

    impl Component for Foo {}
    impl Component for Bar {}
    impl Component for Baz {
        const STORAGE: ComponentStorage = ComponentStorage::Sparse;
    }

    #[test]
    fn drop_dense() {
        static DROP_COUNTER: AtomicUsize = AtomicUsize::new(0);
        struct DropTracker;

        impl Component for DropTracker {
            const STORAGE: ComponentStorage = ComponentStorage::Dense;
        }
        impl Drop for DropTracker {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut world = World::new(WorldIdAllocator::new().alloc());

        // Single
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn(DropTracker).entity;
        DROP_COUNTER.store(0, Ordering::SeqCst);
        world.despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Combined
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world
            .spawn((DropTracker, Bar(3), Baz(String::from("123"))))
            .entity;
        world.despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Repeated
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn((DropTracker, DropTracker, Foo)).entity;
        world.despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn drop_sparse() {
        static DROP_COUNTER: AtomicUsize = AtomicUsize::new(0);
        struct DropTracker;

        impl Component for DropTracker {
            const STORAGE: ComponentStorage = ComponentStorage::Sparse;
        }
        impl Drop for DropTracker {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut world = World::new(WorldIdAllocator::new().alloc());

        // Single
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn(DropTracker).entity;
        DROP_COUNTER.store(0, Ordering::SeqCst);
        world.despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Combined
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world
            .spawn((DropTracker, Bar(3), Baz(String::from("123"))))
            .entity;
        world.despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Repeated
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn((DropTracker, DropTracker, Foo)).entity;
        world.despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn drop_world() {
        static DENSE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        static SPARSE_COUNTER: AtomicUsize = AtomicUsize::new(0);
        struct DenseTracker;
        struct SparseTracker;

        impl Component for DenseTracker {
            const STORAGE: ComponentStorage = ComponentStorage::Dense;
        }
        impl Component for SparseTracker {
            const STORAGE: ComponentStorage = ComponentStorage::Sparse;
        }
        impl Drop for DenseTracker {
            fn drop(&mut self) {
                DENSE_COUNTER.fetch_add(1, Ordering::SeqCst);
            }
        }
        impl Drop for SparseTracker {
            fn drop(&mut self) {
                SPARSE_COUNTER.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut world = World::new(WorldIdAllocator::new().alloc());
        DENSE_COUNTER.store(0, Ordering::SeqCst);
        SPARSE_COUNTER.store(0, Ordering::SeqCst);

        for _ in 0..100 {
            world.spawn(DenseTracker);
            world.spawn((DenseTracker, SparseTracker));
            world.spawn(SparseTracker);
        }

        ::core::mem::drop(world);

        assert_eq!(DENSE_COUNTER.load(Ordering::SeqCst), 200);
        assert_eq!(SPARSE_COUNTER.load(Ordering::SeqCst), 200);
    }
}
