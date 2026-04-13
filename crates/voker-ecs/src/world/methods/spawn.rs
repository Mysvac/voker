use core::iter::FusedIterator;
use core::ptr::NonNull;

use voker_ptr::OwningPtr;

use crate::archetype::Archetype;
use crate::bundle::{Bundle, BundleId};
use crate::component::ComponentWriter;
use crate::entity::{AllocEntitiesIter, Entity, EntityLocation, SpawnError};
use crate::storage::Table;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned, UnsafeWorld, World};

struct BundleSpawner<'a> {
    world: UnsafeWorld<'a>,
    arche: NonNull<Archetype>,
    table: NonNull<Table>,
    write_explicit: unsafe fn(&mut ComponentWriter, usize),
    write_required: unsafe fn(&mut ComponentWriter),
    caller: DebugLocation,
}

impl<'a> BundleSpawner<'a> {
    #[inline(never)]
    fn new(
        world: &'a mut World,
        bundle_id: BundleId,
        write_explicit: unsafe fn(&mut ComponentWriter, usize),
        write_required: unsafe fn(&mut ComponentWriter),
        caller: DebugLocation,
    ) -> BundleSpawner<'a> {
        let arche_id = world.register_archetype(bundle_id);

        let arche = unsafe { world.archetypes.get_unchecked_mut(arche_id) };
        let table_id = arche.table_id();
        let table = unsafe { world.storages.tables.get_unchecked_mut(table_id) };

        BundleSpawner {
            arche: arche.into(),
            table: table.into(),
            world: world.into(),
            write_explicit,
            write_required,
            caller,
        }
    }

    #[inline(never)]
    fn spawn_at(&mut self, data: OwningPtr<'_>, entity: Entity) -> EntityLocation {
        let unsafe_world = self.world;
        let world = unsafe { unsafe_world.full_mut() };

        if ::core::cfg!(debug_assertions) {
            world.entities.can_spawn(entity).unwrap();
        }

        let maps = &mut world.storages.maps;
        let arche = unsafe { self.arche.as_mut() };
        let table = unsafe { self.table.as_mut() };
        let arche_id = arche.id();
        let table_id = table.id();

        let guard = ForgetEntityOnPanic {
            entity,
            world: self.world,
            caller: self.caller,
        };

        let arche_row = unsafe { arche.alloc_row(entity) };
        let table_row = unsafe { table.alloc_row(entity) };
        arche.sparse_components().iter().for_each(|&cid| unsafe {
            let map_id = maps.get_id(cid).debug_checked_unwrap();
            let map = maps.get_unchecked_mut(map_id);
            let _ = map.alloc_row(entity); // `MapRow` may be cached in the future.
        });

        unsafe {
            let mut writer = ComponentWriter::new(world.into(), data, entity, table_id, table_row);

            (self.write_explicit)(&mut writer, 0);
            (self.write_required)(&mut writer);
        }

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

        {
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            arche.trigger_on_add(entity, world.reborrow(), self.caller);
            arche.trigger_on_insert(entity, world.reborrow(), self.caller);
        }

        // We do not flush World here, ensure the location is valid.

        location
    }

    fn alloc(&mut self) -> Entity {
        unsafe { self.world.full_mut().allocator.alloc_mut() }
    }

    fn alloc_many(&mut self, count: u32) -> AllocEntitiesIter<'a> {
        unsafe { self.world.full_mut().allocator.alloc_many(count) }
    }
}

impl World {
    /// Spawns a new entity from a bundle and returns an owned handle to it.
    ///
    /// This method:
    /// - Registers the bundle type (if needed).
    /// - Resolves or creates the matching archetype/table layout.
    /// - Allocates entity storage and writes all explicit/required components.
    ///
    /// The returned [`EntityOwned`] borrows the world and provides convenient
    /// typed access to the spawned entity.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::prelude::*;
    /// #[derive(Component, Clone, Debug)]
    /// struct Foo;
    /// #[derive(Component, Clone, Debug)]
    /// struct Bar(u64);
    ///
    /// let mut world = World::alloc();
    /// let entity = world.spawn((Foo, Bar(123)));
    /// assert!(entity.contains::<(Foo, Bar)>());
    /// ```
    ///
    /// # Note
    ///
    /// Due to lifecycle hooks and their timing, an entity may be consumed
    /// immediately after spawning. As a result, the returned [`EntityOwned`]
    /// may refer to a despawned entity. Use [`EntityOwned::is_spawned`] to
    /// verify whether it is still alive.
    ///
    /// Hooks that immediately despawn their own entity can be useful for
    /// specialized components, but they should not be used as implicit
    /// observers. Component types at spawn time are explicit, while observer
    /// behavior is implicit and can make debugging harder.
    ///
    /// In well-structured codebases, the [`EntityOwned::is_spawned`] check is
    /// often unnecessary.
    #[inline] // We enable inlining to avoid copying data
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> EntityOwned<'_> {
        self.spawn_with_caller(bundle, DebugLocation::caller())
    }

    #[inline]
    pub(crate) fn spawn_with_caller<B: Bundle>(
        &mut self,
        bundle: B,
        caller: DebugLocation,
    ) -> EntityOwned<'_> {
        let bundle_id = self.register_required_bundle::<B>();

        let mut spawner = BundleSpawner::new(
            self,
            bundle_id,
            B::write_explicit,
            B::write_required,
            caller,
        );

        let entity = spawner.alloc();
        voker_ptr::into_owning!(bundle as data);

        let mut location = Some(spawner.spawn_at(data, entity));

        if !self.command_queue.is_empty() {
            self.flush();
            location = self.entities.locate(entity).ok();
        }

        EntityOwned {
            world: self.into(),
            location,
            entity,
        }
    }

    /// Spawns a new entity from a bundle and returns an owned handle to it.
    ///
    /// This method:
    /// - Registers the bundle type (if needed).
    /// - Resolves or creates the matching archetype/table layout.
    /// - Allocates entity storage and writes all explicit/required components.
    ///
    /// The returned [`EntityOwned`] borrows the world and provides convenient
    /// typed access to the spawned entity.
    ///
    /// # Panic
    ///
    /// Panic if the entity is already spawned or invalid generation.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::prelude::*;
    /// #[derive(Component, Clone, Debug)]
    /// struct Foo;
    /// #[derive(Component, Clone, Debug)]
    /// struct Bar(u64);
    ///
    /// let mut world = World::alloc();
    /// let entity = world.alloc_entity();
    /// let entity = world.spawn_at((Foo, Bar(123)), entity).unwrap();
    /// assert!(entity.contains::<(Foo, Bar)>());
    /// ```
    ///
    /// # Note
    ///
    /// Due to lifecycle hooks and their timing, an entity may be consumed
    /// immediately after spawning. As a result, the returned [`EntityOwned`]
    /// may refer to a despawned entity. Use [`EntityOwned::is_spawned`] to
    /// verify whether it is still alive.
    ///
    /// Hooks that immediately despawn their own entity can be useful for
    /// specialized components, but they should not be used as implicit
    /// observers. Component types at spawn time are explicit, while observer
    /// behavior is implicit and can make debugging harder.
    ///
    /// In well-structured codebases, the [`EntityOwned::is_spawned`] check is
    /// often unnecessary.
    #[inline] // We enable inlining to avoid copying data
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn spawn_at<B: Bundle>(
        &mut self,
        bundle: B,
        entity: Entity,
    ) -> Result<EntityOwned<'_>, SpawnError> {
        self.spawn_at_with_caller(bundle, entity, DebugLocation::caller())
    }

    #[inline]
    pub(crate) fn spawn_at_with_caller<B: Bundle>(
        &mut self,
        bundle: B,
        entity: Entity,
        caller: DebugLocation,
    ) -> Result<EntityOwned<'_>, SpawnError> {
        self.entities.can_spawn(entity)?;

        let bundle_id = self.register_required_bundle::<B>();

        let mut spawner = BundleSpawner::new(
            self,
            bundle_id,
            B::write_explicit,
            B::write_required,
            caller,
        );

        voker_ptr::into_owning!(bundle as data);

        let mut location = Some(spawner.spawn_at(data, entity));

        if !self.command_queue.is_empty() {
            self.flush();
            location = self.entities.locate(entity).ok();
        }

        Ok(EntityOwned {
            location,
            world: self.into(),
            entity,
        })
    }
}

pub struct SpawnBatchIter<'w, I>
where
    I: Iterator,
    I::Item: Bundle,
{
    inner: I,
    spawner: BundleSpawner<'w>,
    allocator: AllocEntitiesIter<'w>,
}

impl<I> Drop for SpawnBatchIter<'_, I>
where
    I: Iterator,
    I::Item: Bundle,
{
    fn drop(&mut self) {
        self.by_ref().for_each(|_| {});

        let world = unsafe { self.spawner.world.full_mut() };

        for e in self.allocator.by_ref() {
            world.allocator.free(e);
        }

        world.flush();
    }
}

impl<I> Iterator for SpawnBatchIter<'_, I>
where
    I: Iterator,
    I::Item: Bundle,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        let bundle = self.inner.next()?;
        let entity = self.allocator.next().unwrap_or_else(|| self.spawner.alloc());

        voker_ptr::into_owning!(bundle as data);

        self.spawner.spawn_at(data, entity);

        Some(entity)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I: ExactSizeIterator<Item: Bundle>> ExactSizeIterator for SpawnBatchIter<'_, I> {}
impl<I: FusedIterator<Item: Bundle>> FusedIterator for SpawnBatchIter<'_, I> {}

impl World {
    /// Returns an iterator for batch spawning entities.
    ///
    /// # Important
    ///
    /// If the iterator is not fully consumed, remaining data will
    /// be spawned during `Drop::drop`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::prelude::*;
    /// #[derive(Component, Clone)]
    /// struct Bar(u64);
    ///
    /// let mut world = World::alloc();
    ///
    /// let spawner = world.spawn_batch((0..100_u64).map(|id| Bar(id)));
    /// let entities: Vec<Entity> = spawner.collect();
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn spawn_batch<I, B>(&mut self, iter: I) -> SpawnBatchIter<'_, I::IntoIter>
    where
        B: Bundle,
        I: IntoIterator<Item = B>,
    {
        self.spawn_batch_with_caller(iter, DebugLocation::caller())
    }

    #[inline]
    pub(crate) fn spawn_batch_with_caller<I, B>(
        &mut self,
        iter: I,
        caller: DebugLocation,
    ) -> SpawnBatchIter<'_, I::IntoIter>
    where
        B: Bundle,
        I: IntoIterator<Item = B>,
    {
        let bundle_id = self.register_required_bundle::<B>();
        let mut spawner = BundleSpawner::new(
            self,
            bundle_id,
            B::write_explicit,
            B::write_required,
            caller,
        );

        let inner = iter.into_iter();
        let count = inner.size_hint().0 as u32;
        let allocator = spawner.alloc_many(count);

        SpawnBatchIter {
            inner,
            spawner,
            allocator,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::component::Component;
    use crate::world::World;
    use alloc::string::String;

    #[derive(Component, Clone, Debug, PartialEq, Eq)]
    struct Foo;

    #[derive(Component, Clone, Debug, PartialEq, Eq)]
    struct Bar(u64);

    #[derive(Component, Clone, Debug, PartialEq, Eq)]
    #[component(storage = "sparse")]
    struct Baz(String);

    #[test]
    fn spawn_single() {
        let mut world = World::alloc();

        let entity = world.spawn(Foo);
        assert!(entity.contains::<Foo>());
        assert!(!entity.contains::<Bar>());

        let entity = world.spawn(Bar(123));
        assert_eq!(entity.get::<Bar>(), Some(&Bar(123)));
        assert!(entity.get::<Foo>().is_none());

        let entity = world.spawn(Baz(String::from("hello")));
        assert_eq!(entity.get::<Baz>(), Some(&Baz(String::from("hello"))));
        assert!(entity.get::<Foo>().is_none());
    }

    #[test]
    fn spawn_combined() {
        let mut world = World::alloc();

        let entity = world.spawn((Foo, Bar(123), Baz(String::from("hello"))));
        assert_eq!(entity.get::<Foo>().unwrap(), &Foo);
        assert_eq!(entity.get::<Bar>().unwrap(), &Bar(123));
        assert_eq!(entity.get::<Baz>().unwrap(), &Baz(String::from("hello")));

        // Repeat again to ensure that the access does not change the data.
        assert_eq!(entity.get::<Foo>().unwrap(), &Foo);
        assert_eq!(entity.get::<Bar>().unwrap(), &Bar(123));
        assert_eq!(entity.get::<Baz>().unwrap(), &Baz(String::from("hello")));
    }
}
