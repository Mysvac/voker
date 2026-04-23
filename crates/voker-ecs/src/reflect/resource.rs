//! Reflection adapters for resource operations.
//!
//! This module provides [`ReflectResource`] as ECS resource-oriented type data
//! for runtime scene/asset integration.

use crate::borrow::Mut;
use crate::reflect::from_reflect_with_fallback;
use crate::resource::{Resource, ResourceId};
use crate::utils::DebugName;
use crate::world::World;
use voker_reflect::derive::TypePath;
use voker_reflect::info::Typed;
use voker_reflect::registry::{FromType, TypeRegistry};
use voker_reflect::{FromReflect, Reflect};

/// Runtime reflection adapter for resource types.
#[derive(Clone, TypePath)]
pub struct ReflectResource(ReflectResourceFns);

/// Raw function pointers used by `ReflectResource`.
#[derive(Clone)]
pub struct ReflectResourceFns {
    /// Inserts or replaces a reflected resource value.
    pub insert: fn(&mut World, &dyn Reflect, &TypeRegistry),
    /// Applies reflected data to an existing resource value.
    pub apply: fn(&mut World, &dyn Reflect),
    /// Removes/drops the resource value.
    pub remove: fn(&mut World),
    /// Returns whether the resource currently exists.
    pub contains: fn(&World) -> bool,
    /// Returns shared reflected resource access when present.
    pub reflect: for<'w> fn(&'w World) -> Option<&'w dyn Reflect>,
    /// Returns mutable reflected resource access when present.
    pub reflect_mut: for<'w> fn(&'w mut World) -> Option<Mut<'w, dyn Reflect>>,
    /// Registers the resource type and returns `ResourceId`.
    pub register_resource: fn(&mut World) -> ResourceId,
}

impl ReflectResourceFns {
    /// Builds default function pointers from `T`.
    pub fn new<T>() -> Self
    where
        T: Resource + Reflect + FromReflect + Typed + Send + Sync,
    {
        <ReflectResource as FromType<T>>::from_type().0
    }
}

impl ReflectResource {
    /// Inserts or replaces resource value from reflection.
    #[inline]
    pub fn insert(&self, world: &mut World, value: &dyn Reflect, registry: &TypeRegistry) {
        (self.0.insert)(world, value, registry)
    }

    /// Applies reflected data onto an existing resource.
    ///
    /// # Panics
    /// Panics if the resource does not exist.
    #[inline]
    pub fn apply(&self, world: &mut World, value: &dyn Reflect) {
        (self.0.apply)(world, value)
    }

    /// Removes this resource type from the world.
    #[inline]
    pub fn remove(&self, world: &mut World) {
        (self.0.remove)(world)
    }

    /// Returns whether this resource type exists in the world.
    #[inline]
    pub fn contains(&self, world: &World) -> bool {
        (self.0.contains)(world)
    }

    /// Gets reflected shared access to this resource type.
    #[inline]
    pub fn reflect<'w>(&self, world: &'w World) -> Option<&'w dyn Reflect> {
        (self.0.reflect)(world)
    }

    /// Gets reflected mutable access to this resource type.
    #[inline]
    pub fn reflect_mut<'w>(&self, world: &'w mut World) -> Option<Mut<'w, dyn Reflect>> {
        (self.0.reflect_mut)(world)
    }

    /// Registers this resource type in the world.
    #[inline]
    pub fn register_resource(&self, world: &mut World) -> ResourceId {
        (self.0.register_resource)(world)
    }

    /// Returns low-level function pointers backing this adapter.
    #[inline]
    pub fn fn_pointers(&self) -> &ReflectResourceFns {
        &self.0
    }

    /// Creates a custom resource reflection adapter.
    ///
    /// Most users should rely on `FromType<T>` auto-generation.
    #[inline]
    pub fn new(fns: ReflectResourceFns) -> Self {
        Self(fns)
    }
}

impl<T> FromType<T> for ReflectResource
where
    T: Resource + Reflect + FromReflect + Typed + Send + Sync,
{
    fn from_type() -> Self {
        Self(ReflectResourceFns {
            insert: |world, reflected, registry| {
                let value = from_reflect_with_fallback::<T>(world, reflected, registry);
                world.insert_resource::<T>(value);
            },
            apply: |world, reflected| {
                let mut value = world
                    .get_resource_mut::<T>()
                    .unwrap_or_else(|| cannot_apply(DebugName::type_name::<T>()));
                value.apply(reflected).unwrap();
            },
            remove: World::drop_resource::<T>,
            contains: World::contains_resource::<T>,
            reflect: |world| world.get_resource::<T>().map(|value| value as &dyn Reflect),
            reflect_mut: |world| {
                world
                    .get_resource_mut::<T>()
                    .map(|v| v.map_type(Reflect::as_mut_reflect))
            },
            register_resource: World::register_resource::<T>,
        })
    }
}

#[cold]
#[inline(never)]
fn cannot_apply(name: DebugName) -> ! {
    panic!("cannot apply reflected resource `{name}`: resource is missing")
}
