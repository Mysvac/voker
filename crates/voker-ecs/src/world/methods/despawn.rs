use crate::entity::{DespawnError, Entity, EntityLocation};
use crate::link::LinkHookMode;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, World};

impl World {
    /// Despawns an entity and removes all of its components.
    ///
    /// - Returns `true` if the entity is successfully despawned.
    /// - Returns `false` if the entity does not exist or not spawned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::prelude::*;
    /// # #[derive(Component, Clone)]
    /// # struct Foo;
    /// #
    /// let mut world = World::alloc();
    ///
    /// let entity = world.spawn(Foo).entity();
    /// assert!(world.despawn(entity));
    ///
    /// // Despawning the same entity again returns an error.
    /// assert!(!world.despawn(entity));
    /// ```
    #[inline]
    #[track_caller]
    pub fn despawn(&mut self, entity: Entity) -> bool {
        unsafe {
            let caller = DebugLocation::caller();
            self.entities.set_despawned(entity).is_ok_and(|location| {
                let e = despawn_internal(self, entity, location, caller);
                self.allocator.free(e);
                true
            })
        }
    }

    /// Despawns an entity and removes all of its components.
    ///
    /// Returns an [`DespawnError`] if the entity is not spawned to be despawned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::prelude::*;
    /// # #[derive(Component, Clone)]
    /// # struct Foo;
    /// #
    /// let mut world = World::alloc();
    ///
    /// let entity = world.spawn(Foo).entity();
    /// assert!(world.try_despawn(entity).is_ok());
    ///
    /// // Despawning the same entity again returns an error.
    /// assert!(world.try_despawn(entity).is_err());
    /// ```
    #[inline]
    #[track_caller]
    pub fn try_despawn(&mut self, entity: Entity) -> Result<(), DespawnError> {
        let location = unsafe { self.entities.set_despawned(entity)? };
        let caller = DebugLocation::caller();
        let e = despawn_internal(self, entity, location, caller);
        self.allocator.free(e);
        Ok(())
    }

    /// Despawns an entity and removes all of its components, return new `Entity` handle.
    ///
    /// The new entity handle can be used for [`World::spawn_at`].
    ///
    /// - Returns `Some(new_entity)` if the entity is successfully despawned.
    /// - Returns `None` if the entity does not exist or not spawned.
    #[inline]
    #[track_caller]
    pub fn despawn_no_free(&mut self, entity: Entity) -> Option<Entity> {
        let location = unsafe { self.entities.set_despawned(entity).ok()? };
        let caller = DebugLocation::caller();
        Some(despawn_internal(self, entity, location, caller))
    }

    /// Despawns an entity and removes all of its components, return new `Entity` handle.
    ///
    /// The new entity handle can be used for [`World::spawn_at`].
    ///
    /// - Returns `Ok(new_entity)` if the entity is successfully despawned.
    /// - Returns `Err(DespawnError)` if the entity does not exist or not spawned.
    #[inline]
    #[track_caller]
    pub fn try_despawn_no_free(&mut self, entity: Entity) -> Result<Entity, DespawnError> {
        let location = unsafe { self.entities.set_despawned(entity)? };
        let caller = DebugLocation::caller();
        Ok(despawn_internal(self, entity, location, caller))
    }
}

fn despawn_internal(
    this: &mut World,
    entity: Entity,
    location: EntityLocation,
    caller: DebugLocation,
) -> Entity {
    let unsafe_world = this.unsafe_world();

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
        let link_hook_mode = LinkHookMode::Run;
        arche.trigger_on_despawn(entity, world.reborrow(), link_hook_mode, caller);
        arche.trigger_on_discard(entity, world.reborrow(), link_hook_mode, caller);
        arche.trigger_on_remove(entity, world.reborrow(), link_hook_mode, caller);
    }

    let arche_moved = unsafe { arche.remove_entity(arche_row) };
    let move_res1 = unsafe { world.entities.update_row(arche_moved) };

    let table_id = location.table_id;
    let table_row = location.table_row;
    let table = unsafe { world.storages.tables.get_unchecked_mut(table_id) };
    let table_moved = unsafe { table.swap_remove::<true>(table_row) };
    let move_res2 = unsafe { world.entities.update_row(table_moved) };

    let maps = &mut world.storages.maps;
    arche.sparse_components().iter().for_each(|&cid| unsafe {
        let map_id = maps.get_id(cid).debug_checked_unwrap();
        let map = maps.get_unchecked_mut(map_id);
        let map_row = map.deallocate(entity).unwrap();
        map.drop_item(map_row);
    });

    ::core::mem::forget(guard);

    move_res1.unwrap_or_else(|e| panic!("{e} {caller}"));
    move_res2.unwrap_or_else(|e| panic!("{e} {caller}"));

    // Free before flush.
    let new_entity = unsafe { world.entities.free(entity.id(), 1) };

    world.flush();

    new_entity
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
        world.try_despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Combined
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn((DropTracker, Bar(3), Baz(String::from("123")))).entity;
        world.try_despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Repeated
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn((DropTracker, DropTracker, Foo)).entity;
        world.try_despawn(entity).unwrap();
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
        world.try_despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Combined
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn((DropTracker, Bar(3), Baz(String::from("123")))).entity;
        world.try_despawn(entity).unwrap();
        assert_eq!(DROP_COUNTER.load(Ordering::SeqCst), 1);

        // Repeated
        DROP_COUNTER.store(0, Ordering::SeqCst);
        let entity = world.spawn((DropTracker, DropTracker, Foo)).entity;
        world.try_despawn(entity).unwrap();
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
