use core::any::TypeId;

use crate::bundle::{Bundle, BundleId};
use crate::component::{CollectResult, ComponentCollector};
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
    pub fn get_resource_id<T: Resource>(&self) -> Option<ResourceId> {
        self.resources.get_id(TypeId::of::<T>())
    }

    /// Registers a component type and returns its [`ComponentId`].
    ///
    /// If the type has already been registered, the existing id is returned.
    ///
    /// When you already have `&mut World`, this is a convenient alternative to
    /// [`Components::get_id`].
    ///
    /// This only registers metadata and allocates an id. It does not allocate
    /// storage; storage is prepared lazily during entity insertion.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::component::Component;
    /// # use voker_ecs::world::World;
    /// #[derive(Component)]
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
    /// #[derive(Component)]
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

    /// Try get [`ResourceId`] from specific type.
    ///
    /// If the resource is not registered, the function will return `None`.
    /// When you already have `&mut World`, consider use [`World::register_resource`] instead.
    pub fn get_component_id<T: Component>(&self) -> Option<ComponentId> {
        self.components.get_id(TypeId::of::<T>())
    }

    /// Registers a bundle type and returns its [`BundleId`].
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

            unsafe { world.bundles.register_explicit(type_id, &dense, dense_len) }
        }

        if let Some(id) = self.bundles.get_explicit_id(TypeId::of::<T>()) {
            id
        } else {
            register_cold(self, TypeId::of::<T>(), T::collect_explicit)
        }
    }

    /// Registers a bundle type and returns its [`BundleId`].
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

            unsafe { world.bundles.register_required(type_id, &dense, dense_len) }
        }

        if let Some(id) = self.bundles.get_required_id(TypeId::of::<T>()) {
            id
        } else {
            register_cold(self, TypeId::of::<T>(), T::collect_required)
        }
    }
}
