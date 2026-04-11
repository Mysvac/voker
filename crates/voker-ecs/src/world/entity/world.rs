use alloc::vec::Vec;
use core::mem::MaybeUninit;

use crate::clone::EntityCloner;
use crate::command::Commands;
use crate::entity::{AllocEntitiesIter, Entity, FetchError};
use crate::world::{EntityMut, EntityOwned, EntityRef, UnsafeWorld, World};

macro_rules! once_warning_for_owned {
    () => {
        #[cfg(debug_assertions)]
        voker_os::once_expr!{
            log::warn!{
                "Calling `entity_owned` for multiple entities, consider replace to `entity_mut`: {}.",
                core::panic::Location::caller()
            }
        }
    };
}

impl World {
    /// Allocates a new entity identifier.
    #[inline]
    #[must_use]
    pub fn alloc_entity(&self) -> Entity {
        self.allocator.alloc()
    }

    /// Efficiently allocates multiple entities.
    #[inline]
    #[must_use]
    pub fn alloc_entities(&self, count: usize) -> AllocEntitiesIter<'_> {
        assert!(count < u32::MAX as usize, "too many entities");
        self.allocator.alloc_many(count as u32)
    }

    /// Creates an [`EntityCloner`] bound to this world.
    ///
    /// This is the main entry point for cloning one or more entities while
    /// preserving component-level clone policies.
    ///
    /// # Examples
    /// ```no_run
    /// use voker_ecs::prelude::*;
    ///
    /// #[derive(Component, Clone)]
    /// struct Marker;
    ///
    /// let mut world = World::alloc();
    /// let source = world.spawn(Marker).entity();
    /// let cloned = world.entity_cloner().spawn_clone(source, false);
    ///
    /// assert_ne!(source, cloned);
    /// assert!(world.entity_ref(cloned).contains::<Marker>());
    /// ```
    #[inline]
    pub fn entity_cloner(&mut self) -> EntityCloner<'_> {
        EntityCloner::new(self)
    }

    /// Returns a shared entity view with cached tick context.
    ///
    /// Return `Err(FetchError)` if the entity is not spawned or not exists.
    #[inline]
    pub fn get_entity_ref<E: FetchEntities>(&self, entities: E) -> Result<E::Ref<'_>, FetchError> {
        unsafe { E::fetch_ref(entities, self.unsafe_world()) }
    }

    /// Returns a mutable entity view with cached tick context.
    ///
    /// Return `Err(FetchError)` if the entity is not spawned or not exists.
    #[inline]
    pub fn get_entity_mut<E: FetchEntities>(
        &mut self,
        entities: E,
    ) -> Result<E::Mut<'_>, FetchError> {
        unsafe { E::fetch_mut(entities, self.unsafe_world()) }
    }

    /// Returns an owned entity handle for direct per-entity operations.
    ///
    /// Return `Err(FetchError)` if the entity is not spawned or not exists.
    ///
    /// For multiple entities, this function is equivalent to `get_entity_mut`.
    /// In other world, do **not** call this function for multi-entities.
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn get_entity_owned<E: FetchEntities>(
        &mut self,
        entities: E,
    ) -> Result<E::Owned<'_>, FetchError> {
        unsafe { E::fetch_owned(entities, self.unsafe_world()) }
    }

    /// Returns a shared entity view with cached tick context.
    ///
    /// Similar to `get_entity_ref().unwrap()`.
    ///
    /// # Panics
    /// Panic if fetch failed.
    #[inline]
    pub fn entity_ref<E: FetchEntities>(&self, entities: E) -> E::Ref<'_> {
        self.get_entity_ref::<E>(entities).unwrap()
    }

    /// Returns a mutable entity view with cached tick context.
    ///
    /// Similar to `get_entity_mut().unwrap()`.
    ///
    /// # Panics
    /// Panic if fetch failed.
    #[inline]
    pub fn entity_mut<E: FetchEntities>(&mut self, entities: E) -> E::Mut<'_> {
        self.get_entity_mut::<E>(entities).unwrap()
    }

    /// Returns an owned entity handle for direct per-entity operations.
    ///
    /// For multiple entities, this function is equivalent to `entity_mut`.
    ///
    /// Similar to `get_entity_owned().unwrap()`.
    ///
    /// # Panics
    /// Panic if fetch failed.
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn entity_owned<E: FetchEntities>(&mut self, entities: E) -> E::Owned<'_> {
        self.get_entity_owned::<E>(entities).unwrap()
    }

    /// Simultaneously provides access to entity data and a command queue, which
    /// will be applied when the world is next flushed.
    ///
    /// This allows using borrowed entity data to construct commands where the
    /// borrow checker would otherwise prevent it.
    ///
    /// See [`DeferredWorld::entities_and_commands`] for the deferred version.
    ///
    /// [`DeferredWorld::entities_and_commands`]: crate::world::DeferredWorld::entities_and_commands
    #[inline]
    pub fn entities_and_commands(&mut self) -> (EntityFetcher<'_>, Commands<'_, '_>) {
        let unsafe_world = self.unsafe_world();
        let queue = unsafe { &mut unsafe_world.data_mut().command_queue };
        let fetcher = EntityFetcher(unsafe_world);
        let commands = Commands::new(unsafe { unsafe_world.data_mut() }, queue);
        (fetcher, commands)
    }
}

/// Provides a safe interface for non-structural access to the entities in a [`World`].
///
/// This cannot add or remove components, or spawn or despawn entities,
/// making it relatively safe to access in concert with other ECS data.
/// This type can be constructed via [`World::entities_and_commands`],
/// or [`DeferredWorld::entities_and_commands`].
///
/// [`World::entities_and_commands`]: crate::world::World::entities_and_commands
/// [`DeferredWorld::entities_and_commands`]: crate::world::DeferredWorld::entities_and_commands
#[repr(transparent)]
pub struct EntityFetcher<'w>(UnsafeWorld<'w>);

impl<'w> EntityFetcher<'w> {
    /// Returns a shared entity view with cached tick context.
    ///
    /// Return `Err(FetchError)` if the entity is not spawned or not exists.
    #[inline]
    pub fn get_ref<E: FetchEntities>(&self, entities: E) -> Result<E::Ref<'_>, FetchError> {
        unsafe { E::fetch_ref(entities, self.0) }
    }

    /// Returns a mutable entity view with cached tick context.
    ///
    /// Return `Err(FetchError)` if the entity is not spawned or not exists.
    #[inline]
    pub fn get_mut<E: FetchEntities>(&mut self, entities: E) -> Result<E::Mut<'_>, FetchError> {
        unsafe { E::fetch_mut(entities, self.0) }
    }
}

/// Returns a shared entity view with cached tick context.
fn get_entity_ref(world: &World, entity: Entity) -> Result<EntityRef<'_>, FetchError> {
    let location = world.entities.locate(entity)?;
    let last_run = world.last_run();
    let this_run = world.this_run();
    Ok(EntityRef {
        world: world.unsafe_world(),
        entity,
        location,
        last_run,
        this_run,
    })
}

/// Returns a mutable entity view with cached tick context.
fn get_entity_mut(world: &mut World, entity: Entity) -> Result<EntityMut<'_>, FetchError> {
    let location = world.entities.locate(entity)?;
    let last_run = world.last_run();
    let this_run = world.this_run();
    Ok(EntityMut {
        world: world.unsafe_world(),
        entity,
        location,
        last_run,
        this_run,
    })
}

/// Returns an owned entity handle for direct per-entity operations.
fn get_entity_owned(world: &mut World, entity: Entity) -> Result<EntityOwned<'_>, FetchError> {
    let location = world.entities.locate(entity)?;
    Ok(EntityOwned {
        world: world.into(),
        entity,
        location: Some(location),
    })
}

/// Types that can be used to fetch [`Entity`] references from a [`World`].
///
/// Provided implementations are:
/// - [`Entity`]: Fetch a single entity.
/// - `[Entity; N]`/`&[Entity; N]`: Fetch multiple entities, receiving a
///   same-sized array of references.
/// - `&[Entity]`: Fetch multiple entities, receiving a vector of references.
///
/// Currently, the duplication of entities is not checked, but aliases of
/// mutable references may cause undefined behavior.
///
/// # Safety
///
/// - No aliased mutability is caused by the returned references.
/// - [`FetchEntities::fetch_ref`] returns only read-only references.
/// - [`FetchEntities::fetch_mut`] returns only non-structurally-mutable references.
/// - [`FetchEntities::fetch_owned`] can not return structurally-mutable references
///   for multi-entities.
pub unsafe trait FetchEntities {
    type Ref<'a>;
    type Mut<'a>;
    type Owned<'a>;

    /// # Safety
    /// - The world can be read.
    /// - Returns only read-only references.
    unsafe fn fetch_ref(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Ref<'_>, FetchError>;

    /// # Safety
    /// - The world is non-structurally-mutable.
    /// - Returns only non-structurally-mutable references.
    unsafe fn fetch_mut(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Mut<'_>, FetchError>;

    /// # Safety
    /// - The world is structurally-mutable (exclusive).
    /// - Can **not** return structurally-mutable references for multi-entities.
    unsafe fn fetch_owned(
        this: Self,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Owned<'_>, FetchError>;
}

unsafe impl FetchEntities for Entity {
    type Ref<'a> = EntityRef<'a>;
    type Mut<'a> = EntityMut<'a>;
    type Owned<'a> = EntityOwned<'a>;

    #[inline]
    unsafe fn fetch_ref(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Ref<'_>, FetchError> {
        get_entity_ref(unsafe { world.read_only() }, this)
    }

    #[inline]
    unsafe fn fetch_mut(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Mut<'_>, FetchError> {
        get_entity_mut(unsafe { world.data_mut() }, this)
    }

    #[inline]
    unsafe fn fetch_owned(
        this: Self,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Owned<'_>, FetchError> {
        get_entity_owned(unsafe { world.data_mut() }, this)
    }
}

unsafe impl<const N: usize> FetchEntities for &[Entity; N] {
    type Ref<'a> = [EntityRef<'a>; N];
    type Mut<'a> = [EntityMut<'a>; N];
    type Owned<'a> = [EntityMut<'a>; N];

    unsafe fn fetch_ref(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Ref<'_>, FetchError> {
        let mut result = MaybeUninit::<[EntityRef; N]>::uninit();
        let inner = unsafe { result.assume_init_mut() };
        for (r, &e) in core::iter::zip(inner, this) {
            *r = get_entity_ref(unsafe { world.read_only() }, e)?;
        }
        Ok(unsafe { result.assume_init() })
    }

    unsafe fn fetch_mut(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Mut<'_>, FetchError> {
        let mut result = MaybeUninit::<[EntityMut; N]>::uninit();
        let inner = unsafe { result.assume_init_mut() };
        for (r, &e) in core::iter::zip(inner, this) {
            *r = get_entity_mut(unsafe { world.data_mut() }, e)?;
        }
        Ok(unsafe { result.assume_init() })
    }

    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn fetch_owned(
        this: Self,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Owned<'_>, FetchError> {
        once_warning_for_owned!();
        unsafe { <Self as FetchEntities>::fetch_mut(this, world) }
    }
}

unsafe impl<const N: usize> FetchEntities for [Entity; N] {
    type Ref<'a> = [EntityRef<'a>; N];
    type Mut<'a> = [EntityMut<'a>; N];
    type Owned<'a> = [EntityMut<'a>; N];

    unsafe fn fetch_ref(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Ref<'_>, FetchError> {
        unsafe { <&Self as FetchEntities>::fetch_ref(&this, world) }
    }

    unsafe fn fetch_mut(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Mut<'_>, FetchError> {
        unsafe { <&Self as FetchEntities>::fetch_mut(&this, world) }
    }

    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn fetch_owned(
        this: Self,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Owned<'_>, FetchError> {
        once_warning_for_owned!();
        unsafe { <Self as FetchEntities>::fetch_mut(this, world) }
    }
}

unsafe impl FetchEntities for &[Entity] {
    type Ref<'a> = Vec<EntityRef<'a>>;
    type Mut<'a> = Vec<EntityMut<'a>>;
    type Owned<'a> = Vec<EntityMut<'a>>;

    unsafe fn fetch_ref(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Ref<'_>, FetchError> {
        let mut ret = Vec::with_capacity(this.len());

        for &e in this {
            ret.push(get_entity_ref(unsafe { world.read_only() }, e)?);
        }

        Ok(ret)
    }

    unsafe fn fetch_mut(this: Self, world: UnsafeWorld<'_>) -> Result<Self::Mut<'_>, FetchError> {
        let mut ret = Vec::with_capacity(this.len());

        for &e in this {
            ret.push(get_entity_mut(unsafe { world.data_mut() }, e)?);
        }

        Ok(ret)
    }

    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn fetch_owned(
        this: Self,
        world: UnsafeWorld<'_>,
    ) -> Result<Self::Owned<'_>, FetchError> {
        once_warning_for_owned!();
        unsafe { <Self as FetchEntities>::fetch_mut(this, world) }
    }
}
