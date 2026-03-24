use core::any::TypeId;

use crate::bundle::{Bundle, BundleId};
use crate::component::{CollectResult, Component, ComponentCollector, ComponentId};
use crate::resource::{Resource, ResourceId};
use crate::world::World;

impl World {
    /// Registers a resource type and returns its [`ResourceId`].
    ///
    /// If the type has already been registered, the existing id is returned.
    ///
    /// When you already have `&mut World`, this is a convenient alternative to
    /// [`Resources::get_id`].
    ///
    /// This only registers metadata and allocates an id. It does not allocate
    /// storage; storage is prepared lazily when the resource is inserted.
    ///
    /// [`Resources::get_id`]: crate::resource::Resources::get_id
    #[inline]
    pub fn register_resource<T: Resource>(&mut self) -> ResourceId {
        self.resources.register::<T>()
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
    /// [`Components::get_id`]: crate::component::Components::get_id
    #[inline]
    pub fn register_component<T: Component>(&mut self) -> ComponentId {
        self.components.register::<T>()
    }

    /// Ensures storage slots exist for a resource id.
    ///
    /// If the storage has already been prepared, this is a no-op.
    #[inline]
    pub fn prepare_resource(&mut self, id: ResourceId) {
        if let Some(info) = self.resources.get(id) {
            self.storages.prepare_resource(info);
        }
    }

    /// Ensures storage slots exist for a component id.
    ///
    /// If the storage has already been prepared, this is a no-op.
    ///
    /// At present, this is mainly useful for sparse components, because sparse
    /// storage maps are allocated per component type. Dense components are
    /// allocated per table (component set), so this call has no direct effect.
    #[inline]
    pub fn prepare_component(&mut self, id: ComponentId) {
        if let Some(info) = self.components.get(id) {
            self.storages.prepare_component(info);
        }
    }

    /// Registers a bundle type and returns its [`BundleId`].
    ///
    /// This is called automatically by entity spawning APIs.
    #[inline]
    pub fn register_bundle<T: Bundle>(&mut self) -> BundleId {
        if let Some(id) = self.bundles.get_id_by_type(TypeId::of::<T>()) {
            id
        } else {
            self.register_bundle_slow(TypeId::of::<T>(), T::collect_components)
        }
    }

    #[cold]
    #[inline(never)]
    fn register_bundle_slow(
        &mut self,
        type_id: TypeId,
        register_fn: unsafe fn(&mut ComponentCollector),
    ) -> BundleId {
        let mut collector = ComponentCollector::new(&mut self.components);
        unsafe {
            register_fn(&mut collector);
        }

        let CollectResult {
            mut dense,
            mut sparse,
        } = collector.sorted();

        // 0 <= ComponentId < u32::MAX, so dense_len < u32::MAX.
        let dense_len = dense.len() as u32;

        dense.append(&mut sparse);
        unsafe { self.bundles.register(type_id, &dense, dense_len) }
    }
}
