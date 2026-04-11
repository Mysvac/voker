use core::alloc::Layout;
use core::any::TypeId;
use core::fmt::Debug;

use super::hook::{ComponentHook, ComponentHooks};
use super::{Component, ComponentId, Required, StorageMode};
use crate::clone::ComponentCloner;
use crate::relationship::RelationshipAccessor;
use crate::utils::{DebugName, Dropper};

// -----------------------------------------------------------------------------
// ComponentDescriptor

/// Metadata describing a resource type.
///
/// This descriptor contains all static information about a component type,
/// including its name, type ID, memory layout, and behavior flags.
#[derive(Debug, Clone)]
pub struct ComponentDescriptor {
    pub name: DebugName,
    pub type_id: TypeId,
    pub layout: Layout,
    pub mutable: bool,
    pub no_entity: bool,
    pub storage: StorageMode,
    pub cloner: ComponentCloner,
    pub dropper: Option<Dropper>,
    pub required: Option<Required>,
    pub hooks: ComponentHooks,
}

impl ComponentDescriptor {
    /// Creates a new descriptor for component type `T`.
    pub const fn new<T: Component>() -> Self {
        Self {
            name: DebugName::type_name::<T>(),
            type_id: TypeId::of::<T>(),
            layout: Layout::new::<T>(),
            storage: T::STORAGE,
            mutable: T::MUTABLE,
            no_entity: T::NO_ENTITY,
            dropper: T::DROPPER,
            required: T::REQUIRED,
            cloner: T::CLONER,
            hooks: ComponentHooks {
                on_add: T::ON_ADD,
                on_clone: T::ON_CLONE,
                on_insert: T::ON_INSERT,
                on_remove: T::ON_REMOVE,
                on_discard: T::ON_DISCARD,
                on_despawn: T::ON_DESPAWN,
            },
        }
    }
}

// -----------------------------------------------------------------------------
// ComponentInfo

/// Runtime information for a registered resource.
///
/// Combines a unique [`ComponentId`] with its static [`ComponentDescriptor`].
pub struct ComponentInfo {
    id: ComponentId,
    descriptor: ComponentDescriptor,
    link_accessor: Option<RelationshipAccessor>,
}

impl Debug for ComponentInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Component")
            .field("id", &self.id)
            .field("name", &self.descriptor.name)
            .field("storage", &self.descriptor.storage)
            .field("mutable", &self.descriptor.mutable)
            .finish()
    }
}

impl ComponentInfo {
    /// Creates a new component info with given ID and descriptor.
    #[inline(always)]
    pub(crate) fn new(id: ComponentId, descriptor: ComponentDescriptor) -> Self {
        Self {
            id,
            descriptor,
            link_accessor: None,
        }
    }

    /// Returns the component's unique ID.
    #[inline(always)]
    pub fn id(&self) -> ComponentId {
        self.id
    }

    /// Returns the component's debug name.
    #[inline(always)]
    pub fn name(&self) -> DebugName {
        self.descriptor.name
    }

    /// Returns the component's [`TypeId`].
    #[inline(always)]
    pub fn type_id(&self) -> TypeId {
        self.descriptor.type_id
    }

    /// Returns the component's memory layout.
    #[inline(always)]
    pub fn layout(&self) -> Layout {
        self.descriptor.layout
    }

    /// Returns whether the component can be mutated.
    #[inline(always)]
    pub fn mutable(&self) -> bool {
        self.descriptor.mutable
    }

    /// Returns whether the component does not contain entities.
    #[inline(always)]
    pub fn no_entity(&self) -> bool {
        self.descriptor.no_entity
    }

    /// Returns the component's storage strategy.
    #[inline(always)]
    pub fn storage(&self) -> StorageMode {
        self.descriptor.storage
    }

    /// Returns the function that drops this component, if any.
    #[inline(always)]
    pub fn dropper(&self) -> Option<Dropper> {
        self.descriptor.dropper
    }

    /// Returns the component's clone function.
    #[inline(always)]
    pub fn cloner(&self) -> ComponentCloner {
        self.descriptor.cloner
    }

    /// Returns the component's required implementation.
    #[inline(always)]
    pub fn required(&self) -> Option<Required> {
        self.descriptor.required
    }

    /// Returns the component's `on_add` hook if exists.
    #[inline(always)]
    pub fn on_add(&self) -> Option<ComponentHook> {
        self.descriptor.hooks.on_add
    }

    /// Returns the component's `on_clone` hook if exists.
    #[inline(always)]
    pub fn on_clone(&self) -> Option<ComponentHook> {
        self.descriptor.hooks.on_clone
    }

    /// Returns the component's `on_insert` hook if exists.
    #[inline(always)]
    pub fn on_insert(&self) -> Option<ComponentHook> {
        self.descriptor.hooks.on_insert
    }

    /// Returns the component's `on_remove` hook if exists.
    #[inline(always)]
    pub fn on_remove(&self) -> Option<ComponentHook> {
        self.descriptor.hooks.on_remove
    }

    /// Returns the component's `on_discard` hook if exists.
    #[inline(always)]
    pub fn on_discard(&self) -> Option<ComponentHook> {
        self.descriptor.hooks.on_discard
    }

    /// Returns the component's `on_despawn` hook if exists.
    #[inline(always)]
    pub fn on_despawn(&self) -> Option<ComponentHook> {
        self.descriptor.hooks.on_despawn
    }

    /// Returns the component's `RelationshipAccessor` if exists.
    #[inline(always)]
    pub fn link_accessor(&self) -> Option<RelationshipAccessor> {
        self.link_accessor
    }

    /// Returns a mutable reference to component's hook list.
    ///
    /// It is currently private to ensure that Hook cannot be
    /// modified again after the component has been used.
    #[inline(always)]
    pub(crate) fn hooks_mut(&mut self) -> &mut ComponentHooks {
        &mut self.descriptor.hooks
    }

    /// Set component's link accessor.
    ///
    /// It is currently private that only be used by component registration.
    #[inline(always)]
    pub(crate) fn set_link_accessor(&mut self, accessor: RelationshipAccessor) {
        self.link_accessor = Some(accessor);
    }
}
