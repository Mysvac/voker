//! Reflection adapter for world-based construction.
//!
//! [`ReflectFromWorld`] acts as ECS-side type data that lets runtime code build
//! reflected values from [`World`] context when plain default construction is
//! insufficient.

use alloc::boxed::Box;
use voker_reflect::info::Typed;
use voker_reflect::registry::FromType;
use voker_reflect::{Reflect, info::TypePath};

use crate::world::{FromWorld, World};

/// A struct used to operate on the reflected [`FromWorld`] trait of a type.
///
/// A [`ReflectFromWorld`] for type `T` can be obtained via [`TypeData`].
///
/// [`TypeData`]: voker_reflect::registry::TypeData
#[derive(Clone, TypePath)]
pub struct ReflectFromWorld {
    /// Constructs a reflected value from world context.
    pub from_world: fn(&mut World) -> Box<dyn Reflect>,
}

impl ReflectFromWorld {
    /// Constructs default reflected [`FromWorld`] from world using [`from_world()`](FromWorld::from_world).
    #[inline]
    pub fn from_world(&self, world: &mut World) -> Box<dyn Reflect> {
        (self.from_world)(world)
    }
}

impl<T: Reflect + FromWorld + Typed> FromType<T> for ReflectFromWorld {
    fn from_type() -> Self {
        Self {
            from_world: |world| Box::new(T::from_world(world)),
        }
    }
}
