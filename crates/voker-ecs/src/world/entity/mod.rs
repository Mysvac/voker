// -----------------------------------------------------------------------------
// Modules

mod clear;
mod despawn;
mod fetch_trait;
mod get_trait;
mod insert;
mod remove;
mod world;

pub use fetch_trait::FetchComponents;
pub use get_trait::GetComponents;

// -----------------------------------------------------------------------------
// Inline

use core::fmt::Debug;

use crate::entity::{Entity, EntityLocation};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

/// Owned entity handle tied to a world borrow.
///
/// This is commonly returned by spawn APIs. It supports direct component access
/// and can be converted into [`EntityMut`] or [`EntityRef`].
pub struct EntityOwned<'a> {
    pub(crate) world: UnsafeWorld<'a>,
    pub(crate) entity: Entity,
    pub(crate) location: EntityLocation,
}

/// Mutable entity view with cached tick context.
pub struct EntityMut<'a> {
    pub(crate) world: &'a mut World,
    pub(crate) entity: Entity,
    pub(crate) location: EntityLocation,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

/// Read-only entity view with cached tick context.
pub struct EntityRef<'a> {
    pub(crate) world: &'a World,
    pub(crate) entity: Entity,
    pub(crate) location: EntityLocation,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

// -----------------------------------------------------------------------------
// Entity

impl<'a> From<EntityOwned<'a>> for EntityMut<'a> {
    fn from(value: EntityOwned<'a>) -> Self {
        EntityMut {
            last_run: value.last_run(),
            this_run: value.this_run(),
            world: unsafe { value.world.data_mut() },
            entity: value.entity,
            location: value.location,
        }
    }
}

impl<'a> From<EntityOwned<'a>> for EntityRef<'a> {
    fn from(value: EntityOwned<'a>) -> Self {
        EntityRef {
            last_run: value.last_run(),
            this_run: value.this_run(),
            world: unsafe { value.world.read_only() },
            entity: value.entity,
            location: value.location,
        }
    }
}

impl<'a> From<EntityMut<'a>> for EntityRef<'a> {
    fn from(value: EntityMut<'a>) -> Self {
        EntityRef {
            world: value.world,
            entity: value.entity,
            location: value.location,
            last_run: value.last_run,
            this_run: value.this_run,
        }
    }
}

macro_rules! impl_common_methods {
    ($name:ident) => {
        impl Debug for $name<'_> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("entity", &self.entity)
                    .field("location", &self.location)
                    .finish()
            }
        }

        impl $name<'_> {
            /// Returns the underlying entity id.
            pub fn entity(&self) -> Entity {
                self.entity
            }

            /// Returns whether the entity's archetype contains `T`.
            ///
            /// See [`GetComponents`] for examples.
            pub fn contains<T: GetComponents>(&self) -> bool {
                unsafe { T::contains(self.unsafe_world(), self.location.arche_id) }
            }

            /// Gets raw shared component access for `T`.
            ///
            /// See [`GetComponents`] for examples.
            pub fn get<T: GetComponents>(&self) -> Option<T::Raw<'_>> {
                unsafe {
                    T::get(
                        self.unsafe_world(),
                        self.entity,
                        self.location.table_id,
                        self.location.table_row,
                    )
                }
            }

            /// Gets change-aware shared component access for `T`.
            ///
            /// See [`GetComponents`] for examples.
            pub fn get_ref<T: GetComponents>(&self) -> Option<T::Ref<'_>> {
                unsafe {
                    T::get_ref(
                        self.unsafe_world(),
                        self.entity,
                        self.location.table_id,
                        self.location.table_row,
                        self.last_run(),
                        self.this_run(),
                    )
                }
            }
        }
    };
}

impl_common_methods!(EntityOwned);
impl_common_methods!(EntityMut);
impl_common_methods!(EntityRef);

impl<'a> EntityOwned<'a> {
    #[inline(always)]
    fn this_run(&self) -> Tick {
        unsafe { self.world.full_mut().this_run_fast() }
    }

    #[inline(always)]
    fn last_run(&self) -> Tick {
        unsafe { self.world.read_only().last_run() }
    }

    /// Gets change-aware mutable component access for `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn get_mut<T: GetComponents>(&mut self) -> Option<T::Mut<'_>> {
        unsafe {
            T::get_mut(
                self.world,
                self.entity,
                self.location.table_id,
                self.location.table_row,
                self.last_run(),
                self.this_run(),
            )
        }
    }

    /// Fetches an arbitrary component access pattern described by `T`.
    ///
    /// See [`FetchComponents`] for examples.
    pub fn fetch<T: FetchComponents>(&mut self) -> Option<T::Item<'_>> {
        unsafe {
            T::fetch(
                true,
                self.world,
                self.entity,
                self.location.table_id,
                self.location.table_row,
                self.last_run(),
                self.this_run(),
            )
        }
    }

    pub fn update_location(&mut self) {
        let world = unsafe { self.world.data_mut() };
        self.location = world.entities.locate(self.entity).unwrap_or_else(|err| {
            voker_utils::cold_path();
            let entity = self.entity;
            panic!("Entity {entity} despawned while EntityOwned reference is still held. {err}")
        });
    }

    pub fn world(&self) -> &World {
        unsafe { self.world.read_only() }
    }

    pub fn world_scope<R>(&mut self, func: impl FnOnce(&mut World) -> R) -> R {
        struct Guard<'w, 'a>(&'a mut EntityOwned<'w>);

        impl Drop for Guard<'_, '_> {
            #[inline]
            fn drop(&mut self) {
                self.0.update_location();
            }
        }

        let world = unsafe { self.world.data_mut() };
        let _guard = Guard(self);
        func(world)
    }

    pub fn unsafe_world(&self) -> UnsafeWorld<'_> {
        self.world
    }
}

impl<'a> EntityMut<'a> {
    #[inline(always)]
    fn this_run(&self) -> Tick {
        self.this_run
    }

    #[inline(always)]
    fn last_run(&self) -> Tick {
        self.last_run
    }

    #[inline(always)]
    fn unsafe_world(&self) -> UnsafeWorld<'_> {
        self.world.unsafe_world()
    }

    /// Gets change-aware mutable component access for `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn get_mut<T: GetComponents>(&mut self) -> Option<T::Mut<'_>> {
        unsafe {
            T::get_mut(
                self.world.unsafe_world(),
                self.entity,
                self.location.table_id,
                self.location.table_row,
                self.last_run,
                self.this_run,
            )
        }
    }

    /// Fetches an arbitrary component access pattern described by `T`.
    ///
    /// See [`FetchComponents`] for examples.
    pub fn fetch<T: FetchComponents>(&mut self) -> Option<T::Item<'_>> {
        unsafe {
            T::fetch(
                true,
                self.world.unsafe_world(),
                self.entity,
                self.location.table_id,
                self.location.table_row,
                self.last_run,
                self.this_run,
            )
        }
    }
}

impl<'a> EntityRef<'a> {
    #[inline(always)]
    fn this_run(&self) -> Tick {
        self.this_run
    }

    #[inline(always)]
    fn last_run(&self) -> Tick {
        self.last_run
    }

    #[inline(always)]
    fn unsafe_world(&self) -> UnsafeWorld<'_> {
        self.world.unsafe_world()
    }

    /// Fetches a read-only component access pattern described by `T`.
    ///
    /// If the fetch pattern contains mutable borrows, this method always returns `None`.
    ///
    /// See [`FetchComponents`] for examples.
    pub fn fetch<T: FetchComponents>(&self) -> Option<T::Item<'_>> {
        unsafe {
            T::fetch(
                false,
                self.world.unsafe_world(),
                self.entity,
                self.location.table_id,
                self.location.table_row,
                self.last_run,
                self.this_run,
            )
        }
    }
}
