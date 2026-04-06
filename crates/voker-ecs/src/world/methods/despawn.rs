use crate::entity::{Entity, FetchError};
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
    /// let mut world = World::alloc();
    ///
    /// let entity = world.spawn(Foo).entity();
    /// assert!(world.despawn(entity).is_ok());
    ///
    /// // Despawning the same entity again returns an error.
    /// assert!(world.despawn(entity).is_err());
    /// ```
    #[track_caller]
    pub fn despawn(&mut self, entity: Entity) -> Result<(), FetchError> {
        self.get_entity_owned(entity)?.despawn();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::component::{Component, StorageMode};
    use crate::world::World;
    use alloc::string::String;
    use core::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Foo;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Bar(u64);

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Baz(String);

    impl Component for Foo {}
    impl Component for Bar {}
    impl Component for Baz {
        const STORAGE: StorageMode = StorageMode::Sparse;
    }

    #[test]
    fn drop_dense() {
        static DROP_COUNTER: AtomicUsize = AtomicUsize::new(0);
        #[derive(Clone)]
        struct DropTracker;

        impl Component for DropTracker {
            const STORAGE: StorageMode = StorageMode::Dense;
        }
        impl Drop for DropTracker {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut world = World::alloc();

        // Single
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn(DropTracker).entity;
        DROP_COUNTER.store(0, Ordering::SeqCst);
        world.despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Combined
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn((DropTracker, Bar(3), Baz(String::from("123")))).entity;
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
        #[derive(Clone)]
        struct DropTracker;

        impl Component for DropTracker {
            const STORAGE: StorageMode = StorageMode::Sparse;
        }
        impl Drop for DropTracker {
            fn drop(&mut self) {
                DROP_COUNTER.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut world = World::alloc();

        // Single
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn(DropTracker).entity;
        DROP_COUNTER.store(0, Ordering::SeqCst);
        world.despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Combined
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn((DropTracker, Bar(3), Baz(String::from("123")))).entity;
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

        #[derive(Clone)]
        struct DenseTracker;
        #[derive(Clone)]
        struct SparseTracker;

        impl Component for DenseTracker {
            const STORAGE: StorageMode = StorageMode::Dense;
        }
        impl Component for SparseTracker {
            const STORAGE: StorageMode = StorageMode::Sparse;
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

        let mut world = World::alloc();
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
