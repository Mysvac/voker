use alloc::vec::Vec;
use core::iter::FusedIterator;
use core::ptr::NonNull;

use voker_ptr::OwningPtr;

use crate::archetype::{ArcheId, Archetype};
use crate::bundle::{Bundle, BundleId};
use crate::component::ComponentWriter;
use crate::entity::{AllocEntitiesIter, Entity, EntityLocation};
use crate::storage::Table;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{EntityOwned, UnsafeWorld, World};

struct BundleSpawner<'a> {
    world: UnsafeWorld<'a>,
    arche: NonNull<Archetype>,
    table: NonNull<Table>,
    write_explicit: unsafe fn(&mut ComponentWriter, usize),
    write_required: unsafe fn(&mut ComponentWriter),
    location: DebugLocation,
}

impl<'a> BundleSpawner<'a> {
    #[inline(never)]
    fn new(
        world: &'a mut World,
        bundle_id: BundleId,
        write_explicit: unsafe fn(&mut ComponentWriter, usize),
        write_required: unsafe fn(&mut ComponentWriter),
        location: DebugLocation,
    ) -> BundleSpawner<'a> {
        #[cold]
        #[inline(never)]
        fn register_archetype(world: &mut World, bundle_id: BundleId) -> ArcheId {
            let info = world.bundles.get(bundle_id).unwrap();
            if let Some(id) = world.archetypes.get_id(info.components()) {
                unsafe {
                    world.archetypes.set_bundle_map(bundle_id, id);
                }
                return id;
            }

            let dense_len = info.dense_len();
            let components = info.clone_components();
            let table_id = unsafe {
                let sparses = info.sparse_components();
                world.storages.maps.register(&world.components, sparses);
                let denses = info.dense_components();
                world.storages.tables.register(&world.components, denses)
            };

            unsafe {
                let id = world.archetypes.register(table_id, dense_len, components);
                world.archetypes.set_bundle_map(bundle_id, id);
                id
            }
        }

        let arche_id = world
            .archetypes
            .get_id_by_bundle(bundle_id)
            .unwrap_or_else(|| register_archetype(world, bundle_id));

        let arche = unsafe { world.archetypes.get_unchecked_mut(arche_id) };
        let table_id = arche.table_id();
        let table = unsafe { world.storages.tables.get_unchecked_mut(table_id) };

        BundleSpawner {
            arche: arche.into(),
            table: table.into(),
            world: world.into(),
            write_explicit,
            write_required,
            location,
        }
    }

    #[inline(never)]
    fn spawn_at(&mut self, data: OwningPtr<'_>, entity: Entity) -> EntityLocation {
        let world = unsafe { self.world.full_mut() };

        if ::core::cfg!(debug_assertions) {
            world.entities.can_spawn(entity).unwrap();
        }

        let maps = &mut world.storages.maps;
        let arche = unsafe { self.arche.as_mut() };
        let table = unsafe { self.table.as_mut() };
        let arche_id = arche.id();
        let table_id = arche.table_id();

        let guard = ForgetEntityOnPanic {
            entity,
            world: self.world,
            location: self.location,
        };

        let arche_row = unsafe { arche.insert_entity(entity) };
        let table_row = unsafe { table.allocate(entity) };
        arche.sparse_components().iter().for_each(|&cid| unsafe {
            let map_id = maps.get_id(cid).debug_checked_unwrap();
            let map = maps.get_unchecked_mut(map_id);
            let _ = map.allocate(entity); // `MapRow` may be cached in the future.
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
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct Foo;
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct Bar(u64);
    ///
    /// let mut world = World::alloc();
    /// let entity = world.spawn((Foo, Bar(123)));
    /// assert!(entity.contains::<(Foo, Bar)>());
    /// ```
    #[inline] // We enable inlining to avoid copying data
    #[track_caller]
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> EntityOwned<'_> {
        let bundle_id = self.register_bundle::<B>();

        let mut spawner = BundleSpawner::new(
            self,
            bundle_id,
            B::write_explicit,
            B::write_required,
            DebugLocation::caller(),
        );

        let entity = spawner.alloc();
        voker_ptr::into_owning!(bundle as data);

        EntityOwned {
            location: spawner.spawn_at(data, entity),
            world: self.into(),
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
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct Foo;
    /// #[derive(Component, Debug, PartialEq, Eq)]
    /// struct Bar(u64);
    ///
    /// let mut world = World::alloc();
    /// let entity = world.alloc_entity();
    /// let entity = world.spawn_at((Foo, Bar(123)), entity);
    /// assert!(entity.contains::<(Foo, Bar)>());
    /// ```
    #[inline] // We enable inlining to avoid copying data
    #[track_caller]
    pub fn spawn_at<B: Bundle>(&mut self, bundle: B, entity: Entity) -> EntityOwned<'_> {
        let bundle_id = self.register_bundle::<B>();

        let mut spawner = BundleSpawner::new(
            self,
            bundle_id,
            B::write_explicit,
            B::write_required,
            DebugLocation::caller(),
        );

        voker_ptr::into_owning!(bundle as data);

        EntityOwned {
            location: spawner.spawn_at(data, entity),
            world: self.into(),
            entity,
        }
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
        let len = self.allocator.len();
        if len > 0 {
            let mut buffer = Vec::with_capacity(len);
            self.allocator.by_ref().for_each(|e| buffer.push(e));
            let world = unsafe { self.spawner.world.full_mut() };
            world.allocator.free_many(&buffer);
        }
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
        let entity = self
            .allocator
            .next()
            .unwrap_or_else(|| self.spawner.alloc());

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
    /// Entity **spawning is lazy** and will only execute when the iterator is consumed.
    ///
    /// If the iterator is not fully consumed, remaining data will be properly
    /// released and unused entity IDs will be reclaimed.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::prelude::*;
    /// #[derive(Component)]
    /// struct Bar(u64);
    ///
    /// let mut world = World::alloc();
    ///
    /// let spawner = world.spawn_batch((0..100_u64).map(|id| Bar(id)));
    /// let entities: Vec<Entity> = spawner.collect();
    /// ```
    #[inline]
    #[track_caller]
    #[must_use = "`SpawnBatchIter` is lazy working."]
    pub fn spawn_batch<I, B>(&mut self, iter: I) -> SpawnBatchIter<'_, I::IntoIter>
    where
        B: Bundle,
        I: IntoIterator<Item = B>,
    {
        let bundle_id = self.register_bundle::<B>();
        let mut spawner = BundleSpawner::new(
            self,
            bundle_id,
            B::write_explicit,
            B::write_required,
            DebugLocation::caller(),
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
    use crate::component::{Component, ComponentStorage};
    use crate::world::World;
    use alloc::string::String;

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
