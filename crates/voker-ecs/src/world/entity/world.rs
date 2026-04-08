use alloc::vec::Vec;
use core::mem::MaybeUninit;

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
}

/// Returns a shared entity view with cached tick context.
fn get_entity_ref(world: &World, entity: Entity) -> Result<EntityRef<'_>, FetchError> {
    let location = world.entities.locate(entity)?;
    let last_run = world.last_run();
    let this_run = world.this_run();
    Ok(EntityRef {
        world,
        entity,
        location,
        last_run,
        this_run,
    })
}

/// Returns a mutable entity view with cached tick context.
fn get_entity_mut(world: &mut World, entity: Entity) -> Result<EntityMut<'_>, FetchError> {
    let location = world.entities.locate(entity).unwrap();
    let last_run = world.last_run();
    let this_run = world.this_run();
    Ok(EntityMut {
        world,
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
