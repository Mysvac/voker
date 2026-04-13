use alloc::vec::Vec;
use core::any::TypeId;

use crate::archetype::ArcheId;
use crate::bundle::{Bundle, BundleId};
use crate::component::{CollectResult, ComponentCollector, StorageMode};
use crate::component::{Component, ComponentId};
use crate::resource::{Resource, ResourceId};
use crate::world::World;

impl World {
    /// Registers a resource type and returns its [`ResourceId`].
    ///
    /// If the type has already been registered, the existing id is returned.
    ///
    /// When you already have `&mut World`, this is a convenient alternative to
    /// [`World::get_resource_id`] and [`Resources::get_id`].
    ///
    /// This only registers metadata and allocates an id. It does not allocate
    /// storage; storage is prepared lazily when the resource is inserted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// #[derive(Resource)]
    /// struct Foo;
    ///
    /// let mut world = World::alloc();
    ///
    /// let id_1 = world.register_resource::<Foo>();
    /// let id_2 = world.register_resource::<Foo>();
    /// assert_eq!(id_1, id_2);
    /// ```
    ///
    /// [`Resources::get_id`]: crate::resource::Resources::get_id
    #[inline]
    pub fn register_resource<T: Resource>(&mut self) -> ResourceId {
        self.resources.register::<T>()
    }

    /// Ensures storage slots exist for a resource id.
    ///
    /// If the storage has already been prepared, this is a no-op.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::resource::Resource;
    /// # use voker_ecs::world::World;
    /// #[derive(Resource)]
    /// struct Foo;
    ///
    /// let mut world = World::alloc();
    ///
    /// let id = world.register_resource::<Foo>();
    /// assert!(world.storages.res_set.get(id).is_none());
    ///
    /// world.prepare_resource(id);
    /// assert!(world.storages.res_set.get(id).is_some());
    /// ```
    #[inline]
    pub fn prepare_resource(&mut self, id: ResourceId) {
        if let Some(info) = self.resources.get(id) {
            self.storages.prepare_resource(info);
        }
    }

    /// Try get [`ResourceId`] from specific type.
    ///
    /// If the resource is not registered, the function will return `None`.
    /// When you already have `&mut World`, consider use [`World::register_resource`] instead.
    #[inline]
    pub fn get_resource_id<T: Resource>(&self) -> Option<ResourceId> {
        self.resources.get_id(TypeId::of::<T>())
    }

    /// Registers a component type and returns its [`ComponentId`].
    ///
    /// If the type has already been registered, the existing id is returned.
    ///
    /// When you already have `&mut World`, this is a convenient alternative to
    /// [`World::get_component_id`] and [`Components::get_id`].
    ///
    /// This only registers metadata and allocates an id. It does not allocate
    /// storage; storage is prepared lazily during entity insertion.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::component::Component;
    /// # use voker_ecs::world::World;
    /// #[derive(Component, Clone)]
    /// struct Foo;
    ///
    /// let mut world = World::alloc();
    ///
    /// let id_1 = world.register_component::<Foo>();
    /// let id_2 = world.register_component::<Foo>();
    /// assert_eq!(id_1, id_2);
    /// ```
    ///
    /// [`Components::get_id`]: crate::component::Components::get_id
    #[inline]
    pub fn register_component<T: Component>(&mut self) -> ComponentId {
        self.components.register::<T>()
    }

    /// Ensures storage slots exist for a component id.
    ///
    /// If the storage has already been prepared, this is a no-op.
    ///
    /// At present, this is mainly useful for sparse components, because sparse
    /// storage maps are allocated per component type. Dense components are
    /// allocated per table (component set), so this call has no direct effect.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::component::Component;
    /// # use voker_ecs::world::World;
    /// #[derive(Component, Clone)]
    /// #[component(storage = "sparse")]
    /// struct Foo;
    ///
    /// let mut world = World::alloc();
    ///
    /// let id = world.register_component::<Foo>();
    /// assert!(world.storages.maps.get_id(id).is_none());
    ///
    /// world.prepare_component(id);
    /// assert!(world.storages.maps.get_id(id).is_some());
    /// ```
    #[inline]
    pub fn prepare_component(&mut self, id: ComponentId) {
        if let Some(info) = self.components.get(id) {
            self.storages.prepare_component(info);
        }
    }

    /// Try get [`ComponentId`] from specific type.
    ///
    /// If the component is not registered, the function will return `None`.
    /// When you already have `&mut World`, consider use [`World::register_component`] instead.
    #[inline]
    pub fn get_component_id<T: Component>(&self) -> Option<ComponentId> {
        self.components.get_id(TypeId::of::<T>())
    }

    /// Registers a bundle type and returns its [`BundleId`].
    ///
    /// If the target `BundleInfo` already exists, returns it directly.
    ///
    /// This is called automatically by entity spawning APIs.
    #[inline]
    pub fn register_explicit_bundle<T: Bundle>(&mut self) -> BundleId {
        #[cold]
        #[inline(never)]
        fn register_cold(
            world: &mut World,
            type_id: TypeId,
            collect_explicit: fn(&mut ComponentCollector),
        ) -> BundleId {
            let mut collector = ComponentCollector::new(&mut world.components);
            collect_explicit(&mut collector);

            let CollectResult {
                mut dense,
                mut sparse,
            } = collector.sorted();

            let dense_len = dense.len();
            dense.append(&mut sparse);

            let components = crate::utils::SlicePool::component(&dense);
            unsafe { world.bundles.register_explicit(type_id, components, dense_len) }
        }

        if let Some(id) = self.bundles.get_explicit_id(TypeId::of::<T>()) {
            return id;
        }

        register_cold(self, TypeId::of::<T>(), T::collect_explicit)
    }

    /// Registers a bundle type and returns its [`BundleId`].
    ///
    /// If the target `BundleInfo` already exists, returns it directly.
    ///
    /// This is called automatically by entity spawning APIs.
    #[inline]
    pub fn register_required_bundle<T: Bundle>(&mut self) -> BundleId {
        #[cold]
        #[inline(never)]
        fn register_cold(
            world: &mut World,
            type_id: TypeId,
            collect_required: fn(&mut ComponentCollector),
        ) -> BundleId {
            let mut collector = ComponentCollector::new(&mut world.components);
            collect_required(&mut collector);

            let CollectResult {
                mut dense,
                mut sparse,
            } = collector.sorted();

            let dense_len = dense.len();
            dense.append(&mut sparse);

            let components = crate::utils::SlicePool::component(&dense);
            unsafe { world.bundles.register_required(type_id, components, dense_len) }
        }

        if let Some(id) = self.bundles.get_required_id(TypeId::of::<T>()) {
            return id;
        }

        register_cold(self, TypeId::of::<T>(), T::collect_required)
    }

    /// Registers a bundle from given `ComponentIds` and returns its [`BundleId`].
    ///
    /// If the target `BundleInfo` already exists, returns it directly.
    ///
    /// This function can be used for runtime dynamic operation.
    ///
    /// # Panics
    ///
    /// Panics if any provided component id is not registered in this world.
    pub fn register_dynamic_bundle(&mut self, idents: &[ComponentId]) -> BundleId {
        #[cold]
        #[inline(never)]
        fn register_cold(world: &mut World, idents: &[ComponentId]) -> BundleId {
            let mut dense: Vec<ComponentId> = Vec::with_capacity(idents.len());
            let mut sparse: Vec<ComponentId> = Vec::with_capacity(idents.len());

            idents.iter().for_each(|&id| {
                let info = world.components.get(id).expect("should be reigstered");
                match info.storage() {
                    StorageMode::Dense => dense.push(id),
                    StorageMode::Sparse => sparse.push(id),
                }
            });

            dense.sort();
            dense.dedup();
            sparse.sort();
            sparse.dedup();
            let dense_len = dense.len();
            dense.append(&mut sparse);

            let components = crate::utils::SlicePool::component(&dense);

            unsafe { world.bundles.register_dynamic_unique(components, dense_len) }
        }

        if let Some(id) = self.bundles.get_id(idents) {
            return id;
        }

        register_cold(self, idents)
    }

    /// Registers a archetype from given BundleInfo(ID) and returns its [`ArcheId`].
    ///
    /// If the target `Archetype` already exists, return it's id directly.
    ///
    /// This is called automatically by entity spawning/insertion/.. APIs.
    ///
    /// This function can be coordinated with [`World::register_dynamic_bundle`].
    #[inline]
    pub fn register_archetype(&mut self, bundle_id: BundleId) -> ArcheId {
        #[cold]
        #[inline(never)]
        fn register_cold(world: &mut World, bundle_id: BundleId) -> ArcheId {
            let info = unsafe { world.bundles.get_unchecked(bundle_id) };
            if let Some(id) = world.archetypes.get_id(info.components()) {
                world.archetypes.map_bundle_id(bundle_id, id);
                return id;
            }

            let table_id = unsafe {
                let sparses = info.sparse_components();
                world.storages.maps.register(&world.components, sparses);
                let denses = info.dense_components();
                world.storages.tables.register(&world.components, denses)
            };

            let dense_len = info.dense_components().len();
            let idents = info.components();

            let arche_id = unsafe {
                let components = &world.components;
                world
                    .archetypes
                    .register_unique(table_id, dense_len, idents, components)
            };
            world.archetypes.map_bundle_id(bundle_id, arche_id);

            arche_id
        }

        if let Some(id) = self.archetypes.get_id_by_bundle(bundle_id) {
            return id;
        }

        register_cold(self, bundle_id)
    }
}
