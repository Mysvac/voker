//! Reflection adapters for component operations.
//!
//! This module provides [`ReflectComponent`] as a type-data wrapper that turns
//! statically typed component operations into function-pointer based runtime
//! operations. The wrapper is intended for scene/asset style pipelines where
//! component types are resolved through `TypeRegistry` at runtime.

use voker_reflect::derive::TypePath;
use voker_reflect::info::Typed;
use voker_reflect::registry::{FromType, TypeRegistry};
use voker_reflect::{FromReflect, Reflect};
use voker_utils::debug::DebugName;

use crate::borrow::Mut;
use crate::component::{Component, ComponentId};
use crate::entity::{Entity, EntityMapper};
use crate::reflect::from_reflect_with_fallback;
use crate::world::{EntityMut, EntityOwned, EntityRef, World};

/// Runtime reflection adapter for component types.
#[derive(Clone, TypePath)]
pub struct ReflectComponent(ReflectComponentFns);

/// Raw function pointers used by `ReflectComponent`.
#[derive(Clone)]
pub struct ReflectComponentFns {
    /// Inserts a reflected component into an entity.
    ///
    /// The incoming reflected value is converted to the concrete component
    /// type using reflection fallback rules.
    pub insert: fn(&mut EntityOwned, &dyn Reflect, &TypeRegistry),
    /// Applies a reflected value onto an existing component instance.
    pub apply: fn(&mut EntityMut, &dyn Reflect),
    /// Applies when the component exists, otherwise inserts a mapped value.
    ///
    /// This variant additionally remaps embedded `Entity` references via the
    /// provided mapper before insertion.
    pub apply_or_insert_mapped:
        fn(&mut EntityOwned, &dyn Reflect, &TypeRegistry, &mut dyn EntityMapper),
    /// Removes the component from the entity.
    pub remove: fn(&mut EntityOwned),
    /// Returns `true` if the entity currently contains this component type.
    pub contains: fn(&EntityRef) -> bool,
    /// Returns the component as `&dyn Reflect` when present.
    pub reflect: for<'w> fn(&'w EntityRef) -> Option<&'w dyn Reflect>,
    /// Returns the component as `Mut<dyn Reflect>` when present.
    pub reflect_mut: for<'w> fn(&'w mut EntityMut) -> Option<Mut<'w, dyn Reflect>>,
    /// Remaps embedded `Entity` references in a reflected component value.
    pub map_entities: fn(&mut dyn Reflect, &mut dyn EntityMapper),
    /// Copies the component from source entity/world into destination entity/world.
    pub copy: fn(&World, &mut World, Entity, Entity, &TypeRegistry),
    /// Registers this component type in the world and returns its `ComponentId`.
    pub register_component: fn(&mut World) -> ComponentId,
}

impl ReflectComponentFns {
    /// Builds default function pointers from `T`.
    pub fn new<T>() -> Self
    where
        T: Component + Reflect + FromReflect + Typed,
    {
        <ReflectComponent as FromType<T>>::from_type().0
    }
}

impl ReflectComponent {
    /// Inserts a reflected component value into the target entity.
    ///
    /// The value is converted to the concrete component type before insertion.
    /// Existing component values of the same type are overwritten by ECS insert
    /// semantics.
    #[inline]
    pub fn insert(
        &self,
        entity: &mut EntityOwned,
        component: &dyn Reflect,
        registry: &TypeRegistry,
    ) {
        (self.0.insert)(entity, component, registry)
    }

    /// Applies a reflected value to an existing component instance.
    ///
    /// # Panics
    /// Panics when the component is immutable or missing on the entity.
    #[inline]
    pub fn apply(&self, entity: &mut EntityMut, component: &dyn Reflect) {
        (self.0.apply)(entity, component)
    }

    /// Applies the reflected value if present; otherwise inserts a mapped value.
    ///
    /// This is useful for scene loading where entity references need remapping.
    #[inline]
    pub fn apply_or_insert_mapped(
        &self,
        entity: &mut EntityOwned,
        component: &dyn Reflect,
        registry: &TypeRegistry,
        mapper: &mut dyn EntityMapper,
    ) {
        (self.0.apply_or_insert_mapped)(entity, component, registry, mapper)
    }

    /// Removes this component type from the entity.
    #[inline]
    pub fn remove(&self, entity: &mut EntityOwned) {
        (self.0.remove)(entity)
    }

    /// Returns whether the entity contains this component type.
    #[inline]
    pub fn contains(&self, entity: &EntityRef) -> bool {
        (self.0.contains)(entity)
    }

    /// Gets reflected shared access to this component type.
    #[inline]
    pub fn reflect<'w>(&self, entity: &'w EntityRef) -> Option<&'w dyn Reflect> {
        (self.0.reflect)(entity)
    }

    /// Gets reflected mutable access to this component type.
    ///
    /// # Panics
    /// Panics when the component type is declared immutable.
    #[inline]
    pub fn reflect_mut<'w>(&self, entity: &'w mut EntityMut) -> Option<Mut<'w, dyn Reflect>> {
        (self.0.reflect_mut)(entity)
    }

    /// Remaps embedded entity references in the reflected value.
    ///
    /// This is typically used during scene clone/instantiate flows.
    #[inline]
    pub fn map_entities(&self, reflected: &mut dyn Reflect, mapper: &mut dyn EntityMapper) {
        (self.0.map_entities)(reflected, mapper)
    }

    /// Copies this component from source entity to destination entity.
    ///
    /// Source and destination worlds may differ. Conversion uses reflection
    /// fallback constructors from the registry.
    #[inline]
    pub fn copy(
        &self,
        source_world: &World,
        destination_world: &mut World,
        source_entity: Entity,
        destination_entity: Entity,
        registry: &TypeRegistry,
    ) {
        (self.0.copy)(
            source_world,
            destination_world,
            source_entity,
            destination_entity,
            registry,
        )
    }

    /// Registers this component type in the world.
    #[inline]
    pub fn register_component(&self, world: &mut World) -> ComponentId {
        (self.0.register_component)(world)
    }

    /// Returns low-level function pointers backing this adapter.
    ///
    /// Cloning and storing these pointers can be useful for high-frequency
    /// runtime reflection paths.
    #[inline]
    pub fn fn_pointers(&self) -> &ReflectComponentFns {
        &self.0
    }

    /// Creates a custom component reflection adapter.
    ///
    /// Most users should rely on `FromType<T>` generation; this constructor is
    /// intended for advanced runtime integration scenarios.
    #[inline]
    pub fn new(fns: ReflectComponentFns) -> Self {
        Self(fns)
    }
}

impl<C> FromType<C> for ReflectComponent
where
    C: Component + Reflect + FromReflect + Typed + Send + Sync,
{
    fn from_type() -> Self {
        Self(ReflectComponentFns {
            insert: |entity, reflected, registry| {
                let component = entity.world_scope(|world| {
                    from_reflect_with_fallback::<C>(world, reflected, registry)
                });
                entity.insert(component);
            },
            apply: |entity, reflected| {
                if !C::MUTABLE {
                    cannot_apply(DebugName::type_name::<C>());
                }
                let mut component = entity.get_mut::<C>().unwrap();
                component.apply(reflected).unwrap();
            },
            apply_or_insert_mapped: |entity, reflected, registry, mut mapper| {
                if C::MUTABLE {
                    // SAFETY: guard ensures `C` is a mutable component
                    if let Some(mut component) = entity.get_mut::<C>() {
                        component.apply(reflected).unwrap();
                        if !C::NO_ENTITY {
                            C::map_entities(&mut component, &mut mapper);
                        }
                    } else {
                        let mut component = entity.world_scope(|world| {
                            from_reflect_with_fallback::<C>(world, reflected, registry)
                        });
                        if !C::NO_ENTITY {
                            C::map_entities(&mut component, &mut mapper);
                        }
                        entity.insert(component);
                    }
                } else {
                    let mut component = entity.world_scope(|world| {
                        from_reflect_with_fallback::<C>(world, reflected, registry)
                    });
                    if !C::NO_ENTITY {
                        C::map_entities(&mut component, &mut mapper);
                    }
                    entity.insert(component);
                }
            },
            remove: |entity| {
                entity.remove::<C>();
            },
            contains: |entity| entity.contains::<C>(),
            reflect: |entity| entity.get::<C>().map(|c| c as &dyn Reflect),
            reflect_mut: |entity| {
                if !C::MUTABLE {
                    cannot_get_mut(DebugName::type_name::<C>());
                }
                entity.get_mut::<C>().map(|v| v.map_type(Reflect::as_mut_reflect))
            },
            map_entities: |reflected, mut mapper| {
                if !C::NO_ENTITY {
                    let component = reflected.downcast_mut::<C>().unwrap();
                    Component::map_entities(component, &mut mapper);
                }
            },
            copy: |src_world, dst_world, src, dst, registry| {
                let src_component = src_world.get::<C>(src).unwrap();
                let dst_component =
                    from_reflect_with_fallback::<C>(dst_world, src_component, registry);
                dst_world.entity_owned(dst).insert::<C>(dst_component);
            },
            register_component: World::register_component::<C>,
        })
    }
}

#[cold]
#[inline(never)]
fn cannot_apply(name: DebugName) -> ! {
    panic!("Cannot call `ReflectComponent::apply` on component {name}, it is immutable.")
}

#[cold]
#[inline(never)]
fn cannot_get_mut(name: DebugName) -> ! {
    panic!("Cannot call `ReflectComponent::reflect_mut` on component {name}, it is immutable.")
}
