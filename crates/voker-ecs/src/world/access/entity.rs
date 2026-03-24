use core::fmt::Debug;

use crate::entity::{Entity, EntityError, EntityLocation};
use crate::tick::Tick;
use crate::world::{FetchComponents, GetComponents, UnsafeWorld, World};

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

macro_rules! impl_debug {
    ($name:ident) => {
        impl Debug for $name<'_> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("entity", &self.entity)
                    .field("location", &self.location)
                    .finish()
            }
        }
    };
}

impl_debug!(EntityOwned);
impl_debug!(EntityMut);
impl_debug!(EntityRef);

impl<'a> EntityOwned<'a> {
    #[inline(always)]
    fn this_run(&self) -> Tick {
        let world = unsafe { self.world.data_mut() };
        Tick::new(*world.this_run.get_mut())
    }

    #[inline(always)]
    fn last_run(&self) -> Tick {
        let world = unsafe { self.world.data_mut() };
        Tick::new(*world.this_run.get_mut())
    }

    /// Returns the underlying entity id.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Returns whether the entity's archetype contains `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn contains<T: GetComponents>(&self) -> bool {
        unsafe { T::contains(self.world, self.location.arche_id) }
    }

    /// Gets raw shared component access for `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn get<T: GetComponents>(&self) -> Option<T::Raw<'_>> {
        unsafe {
            T::get(
                self.world,
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
                self.world,
                self.entity,
                self.location.table_id,
                self.location.table_row,
                self.last_run(),
                self.this_run(),
            )
        }
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

    /// Despawns this entity from the world.
    pub fn despawn(self) -> Result<(), EntityError> {
        let world = unsafe { self.world.full_mut() };
        world.despawn(self.entity)
    }
}

impl<'a> EntityMut<'a> {
    /// Returns the underlying entity id.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Returns whether the entity's archetype contains `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn contains<T: GetComponents>(&self) -> bool {
        unsafe { T::contains(self.world.unsafe_world(), self.location.arche_id) }
    }

    /// Gets raw shared component access for `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn get<T: GetComponents>(&self) -> Option<T::Raw<'_>> {
        unsafe {
            T::get(
                self.world.unsafe_world(),
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
                self.world.unsafe_world(),
                self.entity,
                self.location.table_id,
                self.location.table_row,
                self.last_run,
                self.this_run,
            )
        }
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
    /// Returns the underlying entity id.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Returns whether the entity's archetype contains `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn contains<T: GetComponents>(&self) -> bool {
        unsafe { T::contains(self.world.unsafe_world(), self.location.arche_id) }
    }

    /// Gets raw shared component access for `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn get<T: GetComponents>(&self) -> Option<T::Raw<'_>> {
        unsafe {
            T::get(
                self.world.unsafe_world(),
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
                self.world.unsafe_world(),
                self.entity,
                self.location.table_id,
                self.location.table_row,
                self.last_run,
                self.this_run,
            )
        }
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
