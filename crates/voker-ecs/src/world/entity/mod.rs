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
pub use world::{EntityFetcher, FetchEntities};

// -----------------------------------------------------------------------------
// Inline

use core::fmt::Debug;

use voker_ptr::Ptr;

use crate::archetype::Archetype;
use crate::borrow::{UntypedMut, UntypedRef};
use crate::entity::{Entity, EntityLocation};
use crate::prelude::ComponentId;
use crate::tick::Tick;
use crate::utils::{DebugCheckedUnwrap, DebugLocation};
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// EntityMut & EntityRef

/// Mutable entity view with cached tick context.
///
/// The Entity must be spawned.
pub struct EntityMut<'a> {
    pub(crate) world: &'a mut World,
    pub(crate) entity: Entity,
    pub(crate) location: EntityLocation,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
}

/// Read-only entity view with cached tick context.
///
/// The Entity must be spawned.
pub struct EntityRef<'a> {
    pub(crate) world: &'a World,
    pub(crate) entity: Entity,
    pub(crate) location: EntityLocation,
    pub(crate) last_run: Tick,
    pub(crate) this_run: Tick,
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

impl<'a> EntityMut<'a> {
    /// Gets change-aware mutable component reference for `T`.
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

    /// Fetches an arbitrary component reference pattern described by `T`.
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

    /// Gets type-erased change-aware mutable component reference by given ComponentId.
    pub fn get_mut_by_id(&mut self, id: ComponentId) -> Option<UntypedMut<'_>> {
        let arches = &self.world.archetypes;
        let info = unsafe { arches.get_unchecked(self.location.arche_id) };

        if info.contains_dense_component(id) {
            let table_id = self.location.table_id;
            let table_row = self.location.table_row;
            let tables = &mut self.world.storages.tables;
            let table = unsafe { tables.get_unchecked_mut(table_id) };
            let table_col = unsafe { table.get_table_col(id).debug_checked_unwrap() };
            unsafe { Some(table.get_mut(table_row, table_col, self.last_run, self.this_run)) }
        } else if info.contains_sparse_component(id) {
            let maps = &mut self.world.storages.maps;
            let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
            let map = unsafe { maps.get_unchecked_mut(map_id) };
            let map_row = unsafe { map.get_map_row(self.entity).debug_checked_unwrap() };
            unsafe { Some(map.get_mut(map_row, self.last_run, self.this_run)) }
        } else {
            None
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

            /// Returns this entity's location.
            pub fn location(&self) -> EntityLocation {
                self.location
            }

            /// Returns this entity's Archetype.
            pub fn archetype(&self) -> &Archetype {
                let arche_id = self.location.arche_id;
                unsafe { self.world.archetypes.get_unchecked(arche_id) }
            }

            /// Returns whether the entity's archetype contains `T`.
            ///
            /// See [`GetComponents`] for examples.
            pub fn contains<T: GetComponents>(&self) -> bool {
                unsafe { T::contains(self.world.unsafe_world(), self.location.arche_id) }
            }

            /// Gets raw shared component reference for `T`.
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

            /// Gets change-aware shared component reference for `T`.
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

            /// Checks whether the entity contains given Component(Id).
            pub fn contains_by_id(&self, id: ComponentId) -> bool {
                let arches = &self.world.archetypes;
                let info = unsafe { arches.get_unchecked(self.location.arche_id) };
                info.contains_component(id)
            }

            /// Gets raw shared type-erased pointer by given ComponentId.
            pub fn get_by_id(&self, id: ComponentId) -> Option<Ptr<'_>> {
                let arches = &self.world.archetypes;
                let info = unsafe { arches.get_unchecked(self.location.arche_id) };
                if info.contains_dense_component(id) {
                    let table_id = self.location.table_id;
                    let table_row = self.location.table_row;
                    let tables = &self.world.storages.tables;
                    let table = unsafe { tables.get_unchecked(table_id) };
                    let table_col = unsafe { table.get_table_col(id).debug_checked_unwrap() };
                    unsafe { Some(table.get_data(table_row, table_col)) }
                } else if info.contains_sparse_component(id) {
                    let maps = &self.world.storages.maps;
                    let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
                    let map = unsafe { maps.get_unchecked(map_id) };
                    let map_row = unsafe { map.get_map_row(self.entity).debug_checked_unwrap() };
                    unsafe { Some(map.get_data(map_row)) }
                } else {
                    None
                }
            }

            /// Gets type-erased change-aware shared component reference by given ComponentId.
            pub fn get_ref_by_id(&self, id: ComponentId) -> Option<UntypedRef<'_>> {
                let arches = &self.world.archetypes;
                let info = unsafe { arches.get_unchecked(self.location.arche_id) };
                if info.contains_dense_component(id) {
                    let table_id = self.location.table_id;
                    let table_row = self.location.table_row;
                    let tables = &self.world.storages.tables;
                    let table = unsafe { tables.get_unchecked(table_id) };
                    let table_col = unsafe { table.get_table_col(id).debug_checked_unwrap() };
                    unsafe {
                        Some(table.get_ref(table_row, table_col, self.last_run, self.this_run))
                    }
                } else if info.contains_sparse_component(id) {
                    let maps = &self.world.storages.maps;
                    let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
                    let map = unsafe { maps.get_unchecked(map_id) };
                    let map_row = unsafe { map.get_map_row(self.entity).debug_checked_unwrap() };
                    unsafe { Some(map.get_ref(map_row, self.last_run, self.this_run)) }
                } else {
                    None
                }
            }
        }
    };
}

impl_common_methods!(EntityMut);
impl_common_methods!(EntityRef);

// -----------------------------------------------------------------------------
// EntityOwned

/// Owned entity handle tied to a world borrow.
///
/// This is essentially a performance-optimized `(Entity, &mut World)` tuple,
/// which caches the [`EntityLocation`] to reduce duplicate lookups.
///
/// This is commonly returned by spawn APIs. It supports direct component access
/// and can be converted into [`EntityMut`] or [`EntityRef`].
///
/// # Invariants and Risk
///
/// An [`EntityOwned`] may point to a despawned entity. You can check this via
/// [`is_spawned`](Self::is_spawned).
///
/// Unless you have strong reason to assume these invariants, you should generally
/// avoid keeping an [`EntityOwned`] to an entity that is potentially not spawned.
///
/// For example, when inserting a component, that component insert may trigger an
/// observer that despawns the entity. So, when you don't have full knowledge of what
/// commands may interact with this entity, do not further use this value without
/// first checking [`is_spawned`](Self::is_spawned).
pub struct EntityOwned<'a> {
    pub(crate) world: UnsafeWorld<'a>,
    pub(crate) entity: Entity,
    pub(crate) location: Option<EntityLocation>,
}

impl<'a> From<EntityOwned<'a>> for EntityMut<'a> {
    fn from(value: EntityOwned<'a>) -> Self {
        EntityMut {
            location: value.location.unwrap_or_else(|| value.panic_despawned()),
            last_run: value.last_run(),
            this_run: value.this_run(),
            world: unsafe { value.world.data_mut() },
            entity: value.entity,
        }
    }
}

impl<'a> From<EntityOwned<'a>> for EntityRef<'a> {
    fn from(value: EntityOwned<'a>) -> Self {
        EntityRef {
            location: value.location.unwrap_or_else(|| value.panic_despawned()),
            last_run: value.last_run(),
            this_run: value.this_run(),
            world: unsafe { value.world.read_only() },
            entity: value.entity,
        }
    }
}

impl Debug for EntityOwned<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EntityOwned")
            .field("entity", &self.entity)
            .field("location", &self.location)
            .finish()
    }
}

#[repr(transparent)]
struct RelocateGuard<'w, 'a>(&'a mut EntityOwned<'w>);

impl Drop for RelocateGuard<'_, '_> {
    #[inline]
    fn drop(&mut self) {
        self.0.relocate();
    }
}

impl<'a> EntityOwned<'a> {
    #[cold]
    #[track_caller]
    #[inline(never)]
    fn panic_despawned(&self) -> ! {
        let world = unsafe { self.world.read_only() };
        let entity = self.entity;
        let info = world.entities.locate(entity).unwrap_err();
        panic!("`EntityOwned` try operate a despawned Entity({entity}): {info}.");
    }

    #[cold]
    #[inline(never)]
    fn panic_despawned_with_caller(&self, caller: DebugLocation) -> ! {
        let world = unsafe { self.world.read_only() };
        let entity = self.entity;
        let info = world.entities.locate(entity).unwrap_err();
        panic!("`EntityOwned` try operate a despawned Entity({entity}): {info}, {caller}.");
    }

    #[inline(always)]
    pub(crate) fn assert_is_spawned_with_caller(&self, caller: DebugLocation) {
        if self.location.is_none() {
            self.panic_despawned_with_caller(caller)
        }
    }

    #[inline(always)]
    fn this_run(&self) -> Tick {
        unsafe { self.world.full_mut().this_run_fast() }
    }

    #[inline(always)]
    fn last_run(&self) -> Tick {
        unsafe { self.world.read_only().last_run() }
    }

    /// Returns the underlying entity id.
    #[inline(always)]
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Consumes `self` and returns read-only access to all of the entity's
    /// components, with the world `'w` lifetime.
    ///
    /// # Panics
    /// Panics if `self` is despawned.
    #[inline]
    #[track_caller]
    pub fn into_readonly(self) -> EntityRef<'a> {
        EntityRef::from(self)
    }

    /// Consumes `self` and returns non-structural mutable access to all of the
    /// entity's components, with the world `'w` lifetime.
    ///
    /// # Panics
    /// Panics if `self` is despawned.
    #[inline]
    #[track_caller]
    pub fn into_mutable(self) -> EntityMut<'a> {
        EntityMut::from(self)
    }

    /// Gets read-only access to all of the entity's components.
    ///
    /// # Panics
    /// Panics if `self` is despawned.
    #[inline]
    #[track_caller]
    pub fn as_readonly(&self) -> EntityRef<'_> {
        EntityRef {
            location: self.location.unwrap_or_else(|| self.panic_despawned()),
            last_run: self.last_run(),
            this_run: self.this_run(),
            world: unsafe { self.world.read_only() },
            entity: self.entity,
        }
    }

    /// Gets non-structural mutable access to all of the entity's components.
    ///
    /// # Panics
    /// Panics if `self` is despawned.
    #[inline]
    #[track_caller]
    pub fn as_mutable(&mut self) -> EntityMut<'_> {
        EntityMut {
            location: self.location.unwrap_or_else(|| self.panic_despawned()),
            last_run: self.last_run(),
            this_run: self.this_run(),
            world: unsafe { self.world.data_mut() },
            entity: self.entity,
        }
    }

    /// Returns whether the entity's archetype contains `T`.
    ///
    /// Specially, return `false` if the Entity is not spawned.
    ///
    /// See [`GetComponents`] for examples.
    pub fn contains<T: GetComponents>(&self) -> bool {
        if let Some(location) = self.location {
            unsafe { T::contains(self.world, location.arche_id) }
        } else {
            false
        }
    }

    /// Gets raw shared component access for `T`.
    ///
    /// Specially, return `None` if the Entity is not spawned.
    ///
    /// See [`GetComponents`] for examples.
    pub fn get<T: GetComponents>(&self) -> Option<T::Raw<'_>> {
        let location = self.location?;
        unsafe {
            T::get(
                self.world,
                self.entity,
                location.table_id,
                location.table_row,
            )
        }
    }

    /// Gets change-aware shared component access for `T`.
    ///
    /// See [`GetComponents`] for examples.
    pub fn get_ref<T: GetComponents>(&self) -> Option<T::Ref<'_>> {
        let location = self.location?;
        unsafe {
            T::get_ref(
                self.world,
                self.entity,
                location.table_id,
                location.table_row,
                self.last_run(),
                self.this_run(),
            )
        }
    }

    /// Gets change-aware mutable component access for `T`.
    ///
    /// Specially, return `None` if the Entity is not spawned.
    ///
    /// See [`GetComponents`] for examples.
    pub fn get_mut<T: GetComponents>(&mut self) -> Option<T::Mut<'_>> {
        let location = self.location?;
        unsafe {
            T::get_mut(
                self.world,
                self.entity,
                location.table_id,
                location.table_row,
                self.last_run(),
                self.this_run(),
            )
        }
    }

    /// Fetches an arbitrary component access pattern described by `T`.
    ///
    /// Specially, return `None` if the Entity is not spawned.
    ///
    /// See [`FetchComponents`] for examples.
    pub fn fetch<T: FetchComponents>(&mut self) -> Option<T::Item<'_>> {
        let location = self.location?;
        unsafe {
            T::fetch(
                true,
                self.world,
                self.entity,
                location.table_id,
                location.table_row,
                self.last_run(),
                self.this_run(),
            )
        }
    }

    /// Checks whether the entity contains given Component(Id).
    pub fn contains_by_id(&self, id: ComponentId) -> bool {
        let Some(location) = self.location else {
            return false;
        };
        let arches = unsafe { &self.world.read_only().archetypes };
        let info = unsafe { arches.get_unchecked(location.arche_id) };
        info.contains_component(id)
    }

    /// Gets raw shared type-erased pointer by given ComponentId.
    pub fn get_by_id(&self, id: ComponentId) -> Option<Ptr<'_>> {
        let location = self.location?;
        let world = unsafe { self.world.read_only() };

        let arches = &world.archetypes;
        let info = unsafe { arches.get_unchecked(location.arche_id) };
        if info.contains_dense_component(id) {
            let table_id = location.table_id;
            let table_row = location.table_row;
            let tables = &world.storages.tables;
            let table = unsafe { tables.get_unchecked(table_id) };
            let table_col = unsafe { table.get_table_col(id).debug_checked_unwrap() };
            unsafe { Some(table.get_data(table_row, table_col)) }
        } else if info.contains_sparse_component(id) {
            let maps = &world.storages.maps;
            let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
            let map = unsafe { maps.get_unchecked(map_id) };
            let map_row = unsafe { map.get_map_row(self.entity).debug_checked_unwrap() };
            unsafe { Some(map.get_data(map_row)) }
        } else {
            None
        }
    }

    /// Gets type-erased change-aware shared component reference by given ComponentId.
    pub fn get_ref_by_id(&self, id: ComponentId) -> Option<UntypedRef<'_>> {
        let location = self.location?;
        let world = unsafe { self.world.read_only() };
        let last_run = self.last_run();
        let this_run = self.this_run();

        let arches = &world.archetypes;
        let info = unsafe { arches.get_unchecked(location.arche_id) };
        if info.contains_dense_component(id) {
            let table_id = location.table_id;
            let table_row = location.table_row;
            let tables = &world.storages.tables;
            let table = unsafe { tables.get_unchecked(table_id) };
            let table_col = unsafe { table.get_table_col(id).debug_checked_unwrap() };
            unsafe { Some(table.get_ref(table_row, table_col, last_run, this_run)) }
        } else if info.contains_sparse_component(id) {
            let maps = &world.storages.maps;
            let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
            let map = unsafe { maps.get_unchecked(map_id) };
            let map_row = unsafe { map.get_map_row(self.entity).debug_checked_unwrap() };
            unsafe { Some(map.get_ref(map_row, last_run, this_run)) }
        } else {
            None
        }
    }

    /// Gets type-erased change-aware mutable component reference by given ComponentId.
    pub fn get_mut_by_id(&mut self, id: ComponentId) -> Option<UntypedMut<'_>> {
        let location = self.location?;
        let world = unsafe { self.world.data_mut() };
        let last_run = self.last_run();
        let this_run = self.this_run();

        let arches = &world.archetypes;
        let info = unsafe { arches.get_unchecked(location.arche_id) };
        if info.contains_dense_component(id) {
            let table_id = location.table_id;
            let table_row = location.table_row;
            let tables = &mut world.storages.tables;
            let table = unsafe { tables.get_unchecked_mut(table_id) };
            let table_col = unsafe { table.get_table_col(id).debug_checked_unwrap() };
            unsafe { Some(table.get_mut(table_row, table_col, last_run, this_run)) }
        } else if info.contains_sparse_component(id) {
            let maps = &mut world.storages.maps;
            let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
            let map = unsafe { maps.get_unchecked_mut(map_id) };
            let map_row = unsafe { map.get_map_row(self.entity).debug_checked_unwrap() };
            unsafe { Some(map.get_mut(map_row, last_run, this_run)) }
        } else {
            None
        }
    }

    /// Return `true` if the entity is spawned.
    ///
    /// Note that this function check cached [`EntityLocation`] directly,
    /// if you want to update it, call [`EntityOwned::relocate`] before
    /// this function.
    #[inline]
    pub fn is_spawned(&self) -> bool {
        self.location.is_some()
    }

    /// Return the cached [`EntityLocation`].
    ///
    /// if you want to update it, call [`EntityOwned::relocate`] before
    /// this function.
    #[inline]
    pub fn try_location(&self) -> Option<EntityLocation> {
        self.location
    }

    /// Returns the cached archetype that the current entity belongs to.
    ///
    /// if you want to update it, call [`EntityOwned::relocate`] before
    /// this function.
    #[inline]
    pub fn try_archetype(&self) -> Option<&Archetype> {
        let location = self.location?;
        self.world().archetypes.get(location.arche_id)
    }

    /// Return the cached [`EntityLocation`].
    ///
    /// if you want to update it, call [`EntityOwned::relocate`] before
    /// this function.
    ///
    /// # Panics
    /// If the entity has been despawned while this `EntityWorldMut` is still alive.
    #[inline]
    #[track_caller]
    pub fn location(&self) -> EntityLocation {
        match self.location {
            Some(a) => a,
            None => self.panic_despawned(),
        }
    }

    /// Returns the cached archetype that the current entity belongs to.
    ///
    /// if you want to update it, call [`EntityOwned::relocate`] before
    /// this function.
    ///
    /// # Panics
    /// If the entity has been despawned while this `EntityWorldMut` is still alive.
    #[inline]
    #[track_caller]
    pub fn archetype(&self) -> &Archetype {
        match self.location {
            None => self.panic_despawned(),
            Some(a) => unsafe { self.world().archetypes.get_unchecked(a.arche_id) },
        }
    }

    /// Updates the internal entity location to match the current location
    /// in the internal [`World`].
    ///
    /// This is *only* required when using the unsafe function [`EntityOwned::unsafe_world`],
    /// which enables the location to change.
    ///
    /// Note that if the entity is not spawned for any reason, this will have a location of
    /// `None`, leading some methods to panic.
    #[inline]
    pub fn relocate(&mut self) {
        let world = unsafe { self.world.data_mut() };
        self.location = world.entities.locate(self.entity).ok();
    }

    /// Gets read-only access to the world that the current entity belongs to.
    #[inline]
    pub fn world(&self) -> &World {
        unsafe { self.world.read_only() }
    }

    /// Returns this entity's [`World`], consuming itself.
    #[inline]
    pub fn into_world(self) -> &'a mut World {
        unsafe { self.world.full_mut() }
    }

    /// Return the raw handle to [`World`] with manually enforced borrow rules.
    ///
    /// See [`EntityOwned::world_scope`] or [`EntityOwned::into_world`] for a safe alternative.
    ///
    /// # Safety
    ///
    /// If the caller _does_ do something that could change the location, [`EntityOwned::relocate`]
    /// must be called before using any other methods on this [`EntityOwned`].
    #[inline]
    pub fn unsafe_world(&self) -> UnsafeWorld<'_> {
        self.world
    }

    /// Gives mutable access to this entity's [`World`] in a temporary scope.
    ///
    /// This is a safe alternative to using [`EntityOwned::unsafe_world`].
    #[inline]
    pub fn world_scope<R>(&mut self, func: impl FnOnce(&mut World) -> R) -> R {
        let unsafe_world = self.world;
        let _guard = RelocateGuard(self);
        func(unsafe { unsafe_world.data_mut() })
    }

    /// Ensures any commands triggered by the actions
    /// of Self are applied, equivalent to [`World::flush`]
    ///
    /// The commands may despawn this entity, The caller needs to checks
    /// [`EntityOwned::is_spawned`] to ensure safety before other operation.
    #[inline]
    pub fn flush(&mut self) {
        let unsafe_world = self.world;
        let _guard = RelocateGuard(self);
        unsafe {
            unsafe_world.full_mut().flush();
        }
    }
}
