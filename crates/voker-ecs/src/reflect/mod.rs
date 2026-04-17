//! ECS reflection bridge for scene/asset style runtime workflows.
//!
//! This module exposes ECS-focused reflection type-data wrappers that can be
//! stored in `voker_reflect::registry::TypeRegistry`:
//! - `ReflectComponent`: runtime component operations by reflected type.
//! - `ReflectResource`: runtime resource operations by reflected type.
//! - `ReflectMapEntities`: remap embedded `Entity` references during scene load/clone.
//! - `ReflectFromWorld`: world-driven fallback construction for reflected values.
//!
//! Typical runtime usage:
//! 1. collect registrations in [`AppTypeRegistry`],
//! 2. deserialize scene/asset payload into reflected values,
//! 3. dispatch through `ReflectComponent`/`ReflectResource` type data.

mod component;
mod from_world;
mod map_entities;
mod resource;

pub use component::{ReflectComponent, ReflectComponentFns};
pub use from_world::ReflectFromWorld;
pub use map_entities::ReflectMapEntities;
pub use resource::{ReflectResource, ReflectResourceFns};

// -----------------------------------------------------------------------------
// Inline

use alloc::boxed::Box;
use core::any::TypeId;
use core::ops::{Deref, DerefMut};

use voker_reflect::Reflect;
use voker_reflect::info::TypePath;
use voker_reflect::registry::{ReflectDefault, ReflectFromReflect};
use voker_reflect::registry::{TypeRegistry, TypeRegistryArc};

use crate::derive::Resource;
use crate::world::World;

/// App-level type registry resource used by ECS reflection helpers.
///
/// This wraps `TypeRegistryArc` so it can be stored as a normal ECS resource
/// and shared across scene/asset pipelines.
#[derive(Resource, Clone, Default)]
pub struct AppTypeRegistry(pub TypeRegistryArc);

impl Deref for AppTypeRegistry {
    type Target = TypeRegistryArc;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AppTypeRegistry {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AppTypeRegistry {
    /// Registers built-in reflect types from `voker_reflect`.
    ///
    /// Returns `true` if new built-ins were registered during this call.
    pub fn auto_register(&mut self) -> bool {
        self.0.write().auto_register()
    }
}

/// Converts a reflected value into a concrete value using reflected type-data
/// fallbacks registered in `TypeRegistry`.
///
/// Strategy order:
/// 1. same-type fast path via `Reflect::reflect_clone`
/// 2. `ReflectFromReflect`
/// 3. `ReflectDefault` + `apply`
/// 4. `ReflectFromWorld` + `apply`
///
/// Panics if all strategies fail.
pub fn from_reflect_with_fallback<T: Reflect + TypePath>(
    world: &mut World,
    reflected: &dyn Reflect,
    registry: &TypeRegistry,
) -> T {
    #[inline(never)]
    fn from_reflect_impl(
        world: &mut World,
        reflected: &dyn Reflect,
        registry: &TypeRegistry,
        ty_id: TypeId,
        type_path: fn() -> &'static str,
    ) -> Box<dyn Reflect> {
        if reflected.type_id() == ty_id
            && let Ok(value) = reflected.reflect_clone()
        {
            debug_assert_eq! {
                value.as_ref().type_id(), ty_id,
                "reflect_clone produced unexpected type for `{}`",
                type_path(),
            }
            return value;
        }

        if let Some(value) = registry
            .get_type_data::<ReflectFromReflect>(ty_id)
            .and_then(|from_reflect| from_reflect.from_reflect(reflected))
        {
            debug_assert_eq! {
                value.as_ref().type_id(), ty_id,
                "ReflectFromReflect produced unexpected type for `{}`",
                type_path(),
            }
            return value;
        }

        if let Some(ctor) = registry.get_type_data::<ReflectDefault>(ty_id) {
            let mut value = ctor.default();
            debug_assert_eq! {
                value.as_ref().type_id(), ty_id,
                "ReflectDefault produced unexpected type for `{}`",
                type_path(),
            }
            value.apply(reflected).unwrap();
            return value;
        }

        if let Some(ctor) = registry.get_type_data::<ReflectFromWorld>(ty_id) {
            let mut value = ctor.from_world(world);
            debug_assert_eq! {
                value.as_ref().type_id(), ty_id,
                "ReflectFromWorld produced unexpected type for `{}`",
                type_path(),
            }
            value.apply(reflected).unwrap();
            return value;
        }

        panic!(
            "Couldn't create an instance of `{}` using the reflected `Clone` `FromReflect`,\
            `Default`, or `FromWorld` traits. Are you perhaps missing a `#[reflect(Default)]`\
            or `#[reflect(FromWorld)]`?",
            type_path(),
        );
    }

    from_reflect_impl(world, reflected, registry, TypeId::of::<T>(), T::type_path)
        .take()
        .unwrap()
}
