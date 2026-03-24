use voker_ptr::OwningPtr;

use crate::archetype::ArcheId;
use crate::bundle::{Bundle, BundleId};
use crate::component::ComponentWriter;
use crate::entity::{Entity, EntityLocation};
use crate::tick::Tick;
use crate::utils::DebugCheckedUnwrap;
use crate::world::{EntityOwned, World};

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
    /// # use voker_ecs::world::World;
    /// # use voker_ecs::component::Component;
    /// #
    /// # #[derive(Component, Debug, PartialEq, Eq)]
    /// # struct Foo;
    /// # #[derive(Component, Debug, PartialEq, Eq)]
    /// # struct Bar(u64);
    /// #
    /// let mut world = World::default();
    /// let entity = world.spawn((Foo, Bar(123)));
    /// assert!(entity.contains::<(Foo, Bar)>());
    /// ```
    #[inline(always)] // We enable inlining to avoid copying data
    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> EntityOwned<'_> {
        let bundle_id = self.register_bundle::<B>();

        voker_ptr::into_owning!(bundle);
        let entity = self.allocator.alloc_mut();

        self.spawn_internal(
            bundle,
            entity,
            bundle_id,
            B::write_explicit,
            B::write_required,
        )
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
    /// # use voker_ecs::world::World;
    /// # use voker_ecs::component::Component;
    /// # #[derive(Component, Debug, PartialEq, Eq)]
    /// # struct Foo;
    /// # #[derive(Component, Debug, PartialEq, Eq)]
    /// # struct Bar(u64);
    /// let mut world = World::default();
    /// let entity = world.alloc_entity();
    /// let entity = world.spawn_in((Foo, Bar(123)), entity);
    /// assert!(entity.contains::<(Foo, Bar)>());
    /// ```
    #[inline(always)] // We enable inlining to avoid copying data
    pub fn spawn_in<B: Bundle>(&mut self, bundle: B, entity: Entity) -> EntityOwned<'_> {
        let bundle_id = self.register_bundle::<B>();
        voker_ptr::into_owning!(bundle);

        self.spawn_internal(
            bundle,
            entity,
            bundle_id,
            B::write_explicit,
            B::write_required,
        )
    }

    #[inline(never)]
    fn spawn_internal(
        &mut self,
        data: OwningPtr<'_>,
        entity: Entity,
        bundle_id: BundleId,
        write_explicit: unsafe fn(&mut ComponentWriter, usize),
        write_required: unsafe fn(&mut ComponentWriter),
    ) -> EntityOwned<'_> {
        if ::core::cfg!(debug_assertions) {
            self.entities.can_spawn(entity).unwrap();
        }

        let tick = Tick::new(*self.this_run.get_mut());

        let arche_id = self.register_archetype_by_bundle(bundle_id);
        let archetype = unsafe { self.archetypes.get_unchecked_mut(arche_id) };

        let table_id = archetype.table_id();
        let table = unsafe { self.storages.tables.get_unchecked_mut(table_id) };

        let maps = &mut self.storages.maps;
        let components = &self.components;

        for &cid in archetype.sparse_components() {
            unsafe {
                let map_id = maps.get_id(cid).debug_checked_unwrap();
                let map = maps.get_unchecked_mut(map_id);
                let _ = map.allocate(entity); // `MapRow` may be cached in the future.
            }
        }

        let table_row = unsafe { table.allocate(entity) };
        let arche_row = unsafe { archetype.insert_entity(entity) };

        unsafe {
            let mut writer =
                ComponentWriter::new(data, entity, table_row, tick, maps, table, components);

            write_explicit(&mut writer, 0);
            write_required(&mut writer);
        }

        let location = EntityLocation {
            arche_id,
            arche_row,
            table_id,
            table_row,
        };

        unsafe {
            self.entities.set_spawned(entity, location).unwrap();
        }

        EntityOwned {
            world: self.unsafe_world(),
            entity,
            location,
        }
    }

    #[inline]
    fn register_archetype_by_bundle(&mut self, bundle_id: BundleId) -> ArcheId {
        if let Some(id) = self.archetypes.get_id_by_bundle(bundle_id) {
            id
        } else {
            self.register_archetype_by_bundle_slow(bundle_id)
        }
    }

    #[cold]
    #[inline(never)]
    fn register_archetype_by_bundle_slow(&mut self, bundle_id: BundleId) -> ArcheId {
        let info = self.bundles.get(bundle_id).unwrap();
        if let Some(id) = self.archetypes.get_id(info.components()) {
            unsafe {
                self.archetypes.set_bundle_map(bundle_id, id);
            }
            return id;
        }

        let dense_len = info.dense_len();
        let components = info.clone_components();
        let table_id = unsafe {
            let sparses = info.sparse_components();
            self.storages.maps.register(&self.components, sparses);
            let denses = info.dense_components();
            self.storages.tables.register(&self.components, denses)
        };

        unsafe {
            let id = self.archetypes.register(table_id, dense_len, components);
            self.archetypes.set_bundle_map(bundle_id, id);
            id
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
        let mut world = World::default();

        let entity = world.spawn(Foo);
        assert!(entity.get::<Foo>().is_some());
        assert!(entity.get::<Bar>().is_none());

        let entity = world.spawn(Bar(123));
        assert_eq!(entity.get::<Bar>(), Some(&Bar(123)));
        assert!(entity.get::<Foo>().is_none());

        let entity = world.spawn(Baz(String::from("hello")));
        assert_eq!(entity.get::<Baz>(), Some(&Baz(String::from("hello"))));
        assert!(entity.get::<Foo>().is_none());
    }

    #[test]
    fn spawn_combined() {
        let mut world = World::default();

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
