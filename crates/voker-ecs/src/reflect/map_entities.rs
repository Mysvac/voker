//! Reflection adapter for `Entity` remapping.
//!
//! This module exposes [`ReflectMapEntities`] as type data used by scene clone
//! and instantiation flows to remap world-local entity identifiers.

use voker_reflect::derive::TypePath;
use voker_reflect::info::Typed;
use voker_reflect::registry::FromType;
use voker_reflect::{FromReflect, Reflect};

use crate::entity::{EntityMapper, MapEntities};

/// Runtime adapter that remaps `Entity` references inside reflected values.
///
/// See [`EntityMapper`] and [`MapEntities`] for more information.
///
/// [`Entity`]: crate::entity::Entity
/// [`EntityMapper`]: crate::entity::EntityMapper
#[derive(Clone, TypePath)]
pub struct ReflectMapEntities {
    map_entities: fn(&mut dyn Reflect, &mut dyn EntityMapper),
}

impl ReflectMapEntities {
    /// Remaps embedded entities in a reflected value via an [`EntityMapper`].
    ///
    /// This call delegates to the concrete type's `MapEntities` implementation
    /// after downcasting the reflected value.
    ///
    /// # Panics
    /// Panics if the type of the reflected value doesn't match.
    #[inline]
    pub fn map_entities(&self, reflected: &mut dyn Reflect, mapper: &mut dyn EntityMapper) {
        (self.map_entities)(reflected, mapper);
    }
}

impl<C: FromReflect + MapEntities + Typed> FromType<C> for ReflectMapEntities {
    fn from_type() -> Self {
        Self {
            map_entities: |reflected, mut mapper| {
                reflected
                    .downcast_mut::<C>()
                    .expect("reflected type should match")
                    .map_entities(&mut mapper);
            },
        }
    }
}
