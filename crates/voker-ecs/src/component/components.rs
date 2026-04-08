use alloc::vec::Vec;
use core::any::TypeId;
use core::fmt::Debug;

use voker_utils::extra::TypeIdMap;

use super::{Component, ComponentId, ComponentInfo};
use super::{ComponentDescriptor, ComponentRegistrar};

// -----------------------------------------------------------------------------
// Components

/// A registry that manages all component types in the ECS world.
pub struct Components {
    infos: Vec<ComponentInfo>,
    mapper: TypeIdMap<ComponentId>,
}

// -----------------------------------------------------------------------------
// Private

impl Components {
    /// Creates a new empty component registry.
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            infos: Vec::new(),
            mapper: TypeIdMap::new(),
        }
    }

    /// Returns a mutable reference of component info for the given ID without bounds checking.
    ///
    /// It is currently private and only be used for component hook registration.
    ///
    /// # Safety
    /// The caller must ensure `id` is a valid ID (i.e., `id.index() < self.len()`).
    #[inline(always)]
    pub(crate) unsafe fn get_unchecked_mut(&mut self, id: ComponentId) -> &mut ComponentInfo {
        debug_assert!(id.index() < self.infos.len());
        unsafe { self.infos.get_unchecked_mut(id.index()) }
    }
}

// -----------------------------------------------------------------------------
// Basic

impl Debug for Components {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(self.infos.as_slice(), f)
    }
}

impl Components {
    /// Returns the number of registered components.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "useless")]
    pub const fn len(&self) -> usize {
        self.infos.len()
    }

    /// Looks up a component ID by its [`TypeId`].
    #[inline]
    pub fn get_id(&self, type_id: TypeId) -> Option<ComponentId> {
        self.mapper.get(type_id).copied()
    }

    /// Returns the component info for the given ID.
    #[inline]
    pub fn get(&self, id: ComponentId) -> Option<&ComponentInfo> {
        self.infos.get(id.index())
    }

    /// Returns the component info for the given ID without bounds checking.
    ///
    /// # Safety
    /// The caller must ensure `id` is a valid ID (i.e., `id.index() < self.len()`).
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, id: ComponentId) -> &ComponentInfo {
        debug_assert!(id.index() < self.infos.len());
        unsafe { self.infos.get_unchecked(id.index()) }
    }

    /// Extracts a slice containing the entire Components.
    #[inline]
    pub fn as_slice(&self) -> &[ComponentInfo] {
        self.infos.as_slice()
    }

    /// Returns an iterator over the `ComponentInfo` values.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, ComponentInfo> {
        self.infos.iter()
    }

    /// Registers a component type `T` and returns its unique ID.
    ///
    /// If the component type is already registered, this returns the existing ID.
    /// Otherwise, it creates a new descriptor, assigns a new ID, and stores the metadata.
    #[inline]
    pub fn register<T: Component>(&mut self) -> ComponentId {
        // Separate to reduce the stack frame of external functions,
        // thereby improving the execution speed of the hot path.
        #[cold]
        #[inline(never)]
        fn register_cold<T: Component>(this: &mut Components) -> ComponentId {
            let type_id = TypeId::of::<T>();
            let descriptor = ComponentDescriptor::new::<T>();
            let component_id = ComponentId::new(this.infos.len() as u32);

            this.infos.push(ComponentInfo::new(component_id, descriptor));
            this.mapper.insert(type_id, component_id);

            if let Some(required) = T::REQUIRED {
                required.register(&mut ComponentRegistrar::new(this));
            }

            component_id
        }

        if let Some(id) = self.get_id(TypeId::of::<T>()) {
            return id;
        }

        register_cold::<T>(self)
    }
}
