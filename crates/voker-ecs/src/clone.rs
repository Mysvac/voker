//! Entity and component cloning primitives.
//!
//! This module provides the high-level [`EntityCloner`] entry point and the
//! low-level customization types used by [`ComponentCloner::custom`].
//!
//! # Working Model
//! Cloning runs in three phases:
//! - Plain clone: allocate target entities and copy/clone each component.
//! - Deferred remap/callback: remap embedded [`Entity`] references and run
//!   deferred per-component callbacks.
//! - Hooks: trigger lifecycle hooks (`on_clone`, `on_add`, `on_insert`).
//!
//! # Linked And Non-linked Clone
//! The `LINKED` const generic on [`EntityCloner::spawn_clone`] and
//! [`EntityCloner::spawn_clone_batch`] controls whether linked relationship
//! targets should be cloned recursively.
//!
//! # API Layers
//! - [`EntityCloner`]: user-facing entry point for cloning entities.
//! - [`ComponentCloner`]: per-component clone strategy definition.
//! - [`CloneSource`], [`CloneTarget`], [`CloneValue`], [`CloneContext`]:
//!   low-level customization surfaces for advanced cloners.
//!
//! # Examples
//! ```no_run
//! use voker_ecs::prelude::*;
//!
//! #[derive(Component, Clone, Debug, PartialEq, Eq)]
//! struct Health(u32);
//!
//! let mut world = World::alloc();
//! let source = world.spawn(Health(10)).entity();
//!
//! let cloned = world.entity_cloner().spawn_clone(source, false);
//! assert_ne!(source, cloned);
//! assert_eq!(world.entity_ref(cloned).get::<Health>().unwrap().0, 10);
//! ```
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::any::TypeId;
use core::ptr::NonNull;

use voker_ptr::{OwningPtr, Ptr, PtrMut};
use voker_utils::vec::SmallVec;

use crate::component::ComponentId;
use crate::entity::{Entity, EntityHashMap, EntityMapper};
use crate::prelude::Component;
use crate::relationship::{Relationship, RelationshipSourceSet, RelationshipTarget};
use crate::utils::{DebugLocation, DebugName, ForgetEntityOnPanic};
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// CloneSource & CloneTarget & CloneValue

/// Type-erased read-only view of one source component value.
///
/// This is primarily used by custom component cloners.
pub struct CloneSource<'a> {
    ptr: Ptr<'a>,
    name: DebugName,
    type_id: TypeId,
}

/// Type-erased write target for one cloned component value.
///
/// The underlying slot may start as uninitialized memory and must be
/// initialized by the active cloner before returning.
pub struct CloneTarget<'a> {
    ptr: OwningPtr<'a>,
    name: DebugName,
    type_id: TypeId,
    initialized: &'a mut bool,
}

/// Type-erased mutable view used in deferred clone callbacks.
///
/// This is used after plain cloning, when source-target entity mapping is
/// fully available.
pub struct CloneValue<'a> {
    ptr: PtrMut<'a>,
    name: DebugName,
    type_id: TypeId,
}

impl CloneSource<'_> {
    #[cold]
    #[inline(never)]
    fn invalid_type(actual: DebugName, read: DebugName) -> ! {
        panic!("CloneSource Error: try read value as `{read}`, but the actual type is `{actual}`.")
    }

    /// Verifies that this source value has type `C`.
    ///
    /// Panics if the type does not match.
    #[inline(always)]
    pub fn assert_type<C: 'static>(&self) {
        if self.type_id != TypeId::of::<C>() {
            Self::invalid_type(self.name, DebugName::type_name::<C>());
        }
    }

    /// Reads this source value as `C`.
    ///
    /// Panics if the requested type does not match the actual component type.
    pub fn read<C: Sized + 'static>(&self) -> &C {
        self.assert_type::<C>();
        unsafe { self.ptr.deref() }
    }
}

impl CloneTarget<'_> {
    #[cold]
    #[inline(never)]
    fn invalid_type(actual: DebugName, read: DebugName) -> ! {
        panic!("CloneTarget Error: try write value as `{read}`, but the actual type is `{actual}`.")
    }

    /// Verifies that this target slot has type `C`.
    ///
    /// Panics if the type does not match.
    #[inline(always)]
    pub fn assert_type<C: 'static>(&self) {
        if self.type_id != TypeId::of::<C>() {
            Self::invalid_type(self.name, DebugName::type_name::<C>());
        }
    }

    /// Returns whether this target slot is already initialized.
    #[inline(always)]
    pub fn is_initialized(&self) -> bool {
        *self.initialized
    }

    /// Marks this target slot as initialized without writing through [`Self::write`].
    ///
    /// # Safety
    /// Caller must guarantee that the target slot already contains a fully
    /// initialized, valid value for this component type.
    #[inline(always)]
    pub unsafe fn assume_initialized(&mut self) {
        *self.initialized = true;
    }

    /// Writes a cloned component value into this target slot.
    ///
    /// If the slot is already initialized, the previous value is dropped first.
    /// Panics if `C` does not match the target component type.
    pub fn write<C: Sized + 'static>(&mut self, value: C) {
        self.assert_type::<C>();

        if *self.initialized {
            unsafe {
                self.ptr.borrow_mut().promote().drop_as::<C>();
            }
        }

        unsafe {
            self.ptr.write(value);
        }
        *self.initialized = true;
    }
}

impl CloneValue<'_> {
    #[cold]
    #[inline(never)]
    fn invalid_type(actual: DebugName, read: DebugName) -> ! {
        panic!("CloneValue Error: try modify value as `{read}`, but the actual type is `{actual}`.")
    }

    /// Verifies that this value has type `C`.
    ///
    /// Panics if the type does not match.
    #[inline(always)]
    pub fn assert_type<C: 'static>(&self) {
        if self.type_id != TypeId::of::<C>() {
            Self::invalid_type(self.name, DebugName::type_name::<C>());
        }
    }

    /// Mutates this value as `C`.
    ///
    /// This is used in deferred callbacks after plain component cloning.
    pub fn mutate<C: Sized + 'static>(&mut self, fun: impl FnOnce(&mut C)) {
        self.assert_type::<C>();

        unsafe {
            fun(self.ptr.as_mut::<C>());
        }
    }
}

// -----------------------------------------------------------------------------
// CloneContext & CloneEntityMapper & Callback

/// Per-component context passed through a clone run.
///
/// This exposes source/target entities and provides deferred operations for
/// entity remapping and post-clone mutation.
pub struct CloneContext {
    name: DebugName,
    linked_clone: bool,
    id: ComponentId,
    type_id: TypeId,
    source: Entity,
    target: Entity,
    deferred: Vec<Entity>,
    callback: Vec<Callback>,
}

/// Mapping from source entities to their cloned targets.
pub type CloneEntityMapper = EntityHashMap<Entity>;

struct Callback {
    func: Box<dyn FnOnce(CloneValue, &mut CloneEntityMapper)>,
    id: ComponentId,
    entity: Entity,
    name: DebugName,
    type_id: TypeId,
}

impl CloneContext {
    #[cold]
    #[inline(never)]
    fn invalid_type(actual: DebugName, read: DebugName) -> ! {
        panic!(
            "CloneContext Error: try callback value as `{read}`, but the actual type is `{actual}`."
        )
    }

    /// Verifies that the current component type is `C`.
    ///
    /// Panics if the requested type does not match the current clone step.
    #[inline(always)]
    pub fn assert_type<C: 'static>(&self) {
        if self.type_id != TypeId::of::<C>() {
            Self::invalid_type(self.name, DebugName::type_name::<C>());
        }
    }

    pub(crate) fn new(linked_clone: bool) -> Self {
        Self {
            linked_clone,
            id: ComponentId::without_provenance(0),
            source: Entity::PLACEHOLDER,
            target: Entity::PLACEHOLDER,
            type_id: TypeId::of::<Self>(),
            name: DebugName::anonymous(),
            deferred: Vec::new(),
            callback: Vec::new(),
        }
    }

    /// Returns the currently cloned component id.
    pub fn id(&self) -> ComponentId {
        self.id
    }

    /// Returns whether this clone run is in linked mode.
    pub fn linked_clone(&self) -> bool {
        self.linked_clone
    }

    /// Returns the source entity of the current clone step.
    pub fn source_entity(&self) -> Entity {
        self.source
    }

    /// Returns the target entity of the current clone step.
    pub fn target_entity(&self) -> Entity {
        self.target
    }

    /// Schedules another entity to be cloned in the same run.
    ///
    /// This is typically used by relationship-target cloners in linked mode.
    pub fn defer_clone(&mut self, entity: Entity) {
        self.deferred.push(entity);
    }

    /// Schedules deferred entity-remapping for component type `C`.
    ///
    /// This calls [`Component::map_entities`] for the cloned component.
    pub fn defer_map_entities<C: Component>(&mut self) {
        self.assert_type::<C>();
        let wrapper = move |mut value: CloneValue, mapper: &mut CloneEntityMapper| {
            value.mutate::<C>(|c| Component::map_entities(c, mapper))
        };

        self.callback.push(Callback {
            id: self.id,
            entity: self.target,
            func: Box::new(wrapper),
            name: self.name,
            type_id: self.type_id,
        });
    }

    /// Schedules a custom deferred mutation for component type `C`.
    ///
    /// This is useful when cloning needs source-target mapping that is only
    /// available after all target entities have been allocated.
    pub fn defer_mutate<C: Component>(
        &mut self,
        func: impl FnOnce(&mut C, &mut CloneEntityMapper) + Send + 'static,
    ) {
        self.assert_type::<C>();

        let wrapper = move |mut value: CloneValue, mapper: &mut CloneEntityMapper| {
            value.mutate::<C>(|c| func(c, mapper))
        };

        self.callback.push(Callback {
            id: self.id,
            entity: self.target,
            func: Box::new(wrapper),
            name: self.name,
            type_id: self.type_id,
        });
    }
}

// -----------------------------------------------------------------------------
// ComponentCloner

/// Strategy object describing how a single component type is cloned.
///
/// Most component types should use [`Self::copyable`] or [`Self::clonable`].
/// Relationship-aware types should use [`Self::relationship`] or
/// [`Self::relationship_target`].
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct ComponentCloner {
    func: fn(src: CloneSource<'_>, dst: CloneTarget<'_>, ctx: &mut CloneContext),
}

impl ComponentCloner {
    /// Creates a cloner that performs a byte-for-byte copy for `Copy` components.
    ///
    /// If `C::NO_ENTITY` is `false`, deferred entity remapping is queued.
    pub const fn copyable<C: Copy + Component>() -> Self {
        Self {
            func: |src, mut dst, ctx| {
                #[cfg(debug_assertions)]
                {
                    assert!(!dst.is_initialized());
                    src.assert_type::<C>();
                    dst.assert_type::<C>();
                    ctx.assert_type::<C>();
                    src.ptr.debug_assert_aligned::<C>();
                    dst.ptr.debug_assert_aligned::<C>();
                }

                unsafe {
                    dst.assume_initialized();
                    let src = src.ptr.as_ptr() as *const C;
                    let dst = dst.ptr.as_ptr() as *mut C;
                    core::ptr::copy_nonoverlapping::<C>(src, dst, 1);
                }

                if !C::NO_ENTITY {
                    ctx.defer_map_entities::<C>();
                }
            },
        }
    }

    /// Creates a cloner that calls [`Clone::clone`] for components.
    ///
    /// If `C::NO_ENTITY` is `false`, deferred entity remapping is queued.
    pub const fn clonable<C: Clone + Component>() -> Self {
        Self {
            func: |src, mut dst, ctx| {
                #[cfg(debug_assertions)]
                {
                    assert!(!dst.is_initialized());
                    src.assert_type::<C>();
                    dst.assert_type::<C>();
                    ctx.assert_type::<C>();
                    src.ptr.debug_assert_aligned::<C>();
                    dst.ptr.debug_assert_aligned::<C>();
                }

                unsafe {
                    dst.ptr.write::<C>(src.ptr.deref::<C>().clone());
                    dst.assume_initialized();
                }

                if !C::NO_ENTITY {
                    ctx.defer_map_entities::<C>();
                }
            },
        }
    }

    /// Creates a relationship-target-aware cloner.
    ///
    /// In linked mode, linked children are deferred for recursive cloning.
    /// In non-linked mode, linked source sets are cleared when required by the
    /// relationship target policy.
    pub const fn relationship_target<R: Clone + RelationshipTarget>() -> Self {
        Self {
            func: |src, mut dst, ctx| {
                debug_assert!(
                    !R::NO_ENTITY,
                    "RelationshipTarget `{}` cannot annotate `NO_ENTITY`.",
                    DebugName::type_name::<R>(),
                );

                let mut value = src.read::<R>().clone();
                if R::LINKED_LIFECYCLE {
                    if ctx.linked_clone {
                        for child in value.iter() {
                            ctx.defer_clone(child);
                        }
                        dst.write(value);
                    } else {
                        R::raw_sources_mut(&mut value).clear();
                        dst.write(value);
                    }
                } else {
                    dst.write(value);
                }

                ctx.defer_map_entities::<R>();
            },
        }
    }

    /// Creates a relationship-aware cloner.
    ///
    /// Relationship components are cloned and then deferred for entity remapping.
    pub const fn relationship<R: Clone + Relationship>() -> Self {
        Self {
            func: |src, mut dst, ctx| {
                debug_assert!(
                    !R::NO_ENTITY,
                    "Relationship `{}` cannot annotate `NO_ENTITY`.",
                    DebugName::type_name::<R>(),
                );

                dst.write::<R>(src.read::<R>().clone());

                ctx.defer_map_entities::<R>();
            },
        }
    }

    /// Creates a fully custom cloner.
    ///
    /// Most users should prefer [`Self::copyable`] or [`Self::clonable`].
    ///
    /// A custom cloner should always initialize `dst` and should queue remap
    /// when the component contains embedded entities.
    pub const fn custom(func: fn(CloneSource, CloneTarget, &mut CloneContext)) -> Self {
        Self { func }
    }

    /// Invokes this cloner.
    #[inline(always)]
    pub fn call(
        self,
        source: CloneSource<'_>,
        target: CloneTarget<'_>,
        context: &mut CloneContext,
    ) {
        (self.func)(source, target, context)
    }
}

// -----------------------------------------------------------------------------
// EntityCloner

/// High-level entity cloning entry point.
///
/// Create this via [`World::entity_cloner`], then call
/// [`Self::spawn_clone`] or [`Self::spawn_clone_batch`].
pub struct EntityCloner<'w> {
    world: UnsafeWorld<'w>,
    mapper: CloneEntityMapper,
    cloned: Vec<Entity>,
    wait: VecDeque<Entity>,
}

impl<'w> EntityCloner<'w> {
    /// Creates an entity cloner bound to the given world.
    pub fn new(world: &mut World) -> EntityCloner<'_> {
        EntityCloner {
            world: world.unsafe_world(),
            mapper: EntityHashMap::new(),
            cloned: Vec::new(),
            wait: VecDeque::new(),
        }
    }

    /// Clones a batch of entities.
    ///
    /// The returned vector preserves input order and contains cloned target
    /// entities for each input source entity.
    ///
    /// If `LINKED` is `true`, linked entities may be recursively cloned when
    /// relationship cloners request it.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_ecs::prelude::*;
    ///
    /// #[derive(Component, Clone, Debug, PartialEq, Eq)]
    /// struct Health(u32);
    ///
    /// let mut world = World::alloc();
    /// let a = world.spawn(Health(1)).entity();
    /// let b = world.spawn(Health(2)).entity();
    ///
    /// let cloned = world.entity_cloner().spawn_clone_batch(&[a, b], false);
    /// assert_eq!(cloned.len(), 2);
    /// assert_eq!(world.entity_ref(cloned[0]).get::<Health>().unwrap().0, 1);
    /// assert_eq!(world.entity_ref(cloned[1]).get::<Health>().unwrap().0, 2);
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn spawn_clone_batch(&mut self, entities: &[Entity], linked_clone: bool) -> Vec<Entity> {
        let caller = DebugLocation::caller();
        self.wait.extend(entities);
        self.run(linked_clone, caller).into_vec()
    }

    /// Clones one entity and returns the cloned target entity id.
    ///
    /// If `LINKED` is `true`, linked entities may be recursively cloned when
    /// relationship cloners request it.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_ecs::prelude::*;
    ///
    /// #[derive(Component, Clone, Debug, PartialEq, Eq)]
    /// struct NameTag(&'static str);
    ///
    /// let mut world = World::alloc();
    /// let source = world.spawn(NameTag("source")).entity();
    /// let target = world.entity_cloner().spawn_clone(source, false);
    ///
    /// assert_ne!(source, target);
    /// assert_eq!(world.entity_ref(target).get::<NameTag>().unwrap().0, "source");
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn spawn_clone(&mut self, entity: Entity, linked_clone: bool) -> Entity {
        let caller = DebugLocation::caller();
        self.wait.push_back(entity);
        self.run(linked_clone, caller)[0]
    }

    #[inline(never)]
    fn run(&mut self, linked_clone: bool, caller: DebugLocation) -> SmallVec<Entity, 2> {
        let mut context = CloneContext::new(linked_clone);

        // Store entities that are explicitly cloned.
        let mut output: SmallVec<Entity, 2> = SmallVec::with_capacity(self.wait.len());
        for &e in self.wait.iter() {
            unsafe {
                output.push_unchecked(e);
            }
        }

        // -------------------------------------------------------------------
        // Forget Guard
        // -------------------------------------------------------------------

        // If the program panic, we need to forget all cloned entities.
        struct ForgetGuard<'a> {
            world: UnsafeWorld<'a>,
            cloned: NonNull<Vec<Entity>>,
            caller: DebugLocation,
        }

        impl Drop for ForgetGuard<'_> {
            #[cold]
            #[inline(never)]
            fn drop(&mut self) {
                let world = unsafe { self.world.full_mut() };
                let entities = unsafe { self.cloned.as_mut().as_slice() };
                for &entity in entities {
                    unsafe {
                        world.forget_with_caller(entity, self.caller);
                    }
                }
            }
        }

        let forget_guard = ForgetGuard {
            world: self.world,
            cloned: NonNull::from(&self.cloned),
            caller,
        };

        // -------------------------------------------------------------------
        // Plain Clone
        // -------------------------------------------------------------------

        // Clone all waiting entities.
        while let Some(source) = self.wait.pop_front() {
            let world = unsafe { self.world.full_mut() };

            // Obtain the Archetype Info of the source entity.
            let arche_id = match world.entities.locate(source) {
                Ok(location) => location.arche_id,
                Err(e) => {
                    voker_utils::cold_path();
                    log::warn!("Try Clone Entity `{source}` but it is not spawned. {e}. {caller}");
                    continue;
                }
            };

            // Spawn a uninitialized entity from given Archetype.
            let mut uninit_entity = unsafe { world.spawn_uninit_with_caller(arche_id, caller) };

            // `ForgetGuard` can not forget this cloning entity.
            // We need handle it manually.
            let item_guard = ForgetEntityOnPanic {
                entity: uninit_entity.entity,
                world: self.world,
                caller,
            };

            // new entity should not be source entity.
            debug_assert_ne!(uninit_entity.entity(), source);

            context.source = source;
            context.target = uninit_entity.entity;

            // Reloade Archetype and Source Entity Infomation
            // We must reload it because `spawn_uninit_with_caller` may change
            // the structure of the world. (Although it seems unlikely).
            let world = unsafe { self.world.read_only() };
            let source_entity = world.entity_ref(source);
            let arche = source_entity.archetype();
            debug_assert_eq!(arche.id(), source_entity.location.arche_id);
            debug_assert_eq!(arche.id(), uninit_entity.location.arche_id);

            // Plain clone all components through it's `ComponentCloner`.
            for &id in arche.components() {
                let info = unsafe { world.components.get_unchecked(id) };

                let name = info.name();
                let type_id = info.type_id();
                let cloner = info.cloner();

                context.id = id;
                context.name = name;
                context.type_id = type_id;

                let src_ptr = source_entity.get_by_id(id).unwrap();
                let dst_ptr = uninit_entity.get_mut_by_id(id).unwrap().value;

                let src = CloneSource {
                    ptr: src_ptr,
                    name,
                    type_id,
                };

                let mut initialized = false;
                let dst = CloneTarget {
                    ptr: unsafe { dst_ptr.promote() },
                    name,
                    type_id,
                    initialized: &mut initialized,
                };

                cloner.call(src, dst, &mut context);

                // The `CloneTarget` must be initialized through `CloneTarget::write`.
                // For some special types, such as data buffers, consider mark initialized
                // through `CloneTarget::assume_initialized`, but it's unrecommand.
                assert!(
                    initialized,
                    "The ComponentCloner of `{name}<{type_id:?}>` did not write data. {}",
                    caller,
                )
            }

            self.mapper.set_mapped(source, uninit_entity.entity);
            self.cloned.push(uninit_entity.entity);

            ::core::mem::forget(item_guard);

            // Collect all entities that should be linked clone.
            // Note that the input `linked_clone` is non mandatory.
            context.deferred.drain(..).for_each(|entity| {
                use crate::utils::contains_entity;
                let (x, y) = self.wait.as_slices();
                let c1 = !contains_entity(entity, x);
                let c2 = !contains_entity(entity, y);
                let c3 = !contains_entity(entity, &self.cloned);
                let c4 = !self.mapper.contains_key(&entity);
                if c1 && c2 && c3 && c4 {
                    self.wait.push_back(entity);
                }
            });
        }

        // -------------------------------------------------------------------
        // Callbacks
        // -------------------------------------------------------------------

        // Run callbacks
        let callbacks = context.callback;
        let world = unsafe { self.world.full_mut() };
        for callback in callbacks {
            let Callback {
                func,
                id,
                entity,
                name,
                type_id,
            } = callback;

            // The cloning operation has not yet called the lifecycle hooks.
            // The target entity should exist.
            let mut entity_mut = world.get_entity_mut(entity).unwrap();
            let untyped = entity_mut.get_mut_by_id(id).expect("should exist");
            let ptr = untyped.value;
            let clone_value = CloneValue { ptr, name, type_id };
            func(clone_value, &mut self.mapper);
        }

        // -------------------------------------------------------------------
        // Component Hooks
        // -------------------------------------------------------------------

        // Run Lifetime Hooks
        let world = unsafe { self.world.full_mut() };
        for &entity in self.cloned.as_slice() {
            if let Ok(location) = world.entities.locate(entity) {
                let arche_id = location.arche_id;
                let arche_info = unsafe { world.archetypes.get_unchecked(arche_id) };
                let mut deferred = unsafe { self.world.deferred() };

                arche_info.trigger_on_clone(entity, deferred.reborrow(), caller);
                arche_info.trigger_on_add(entity, deferred.reborrow(), caller);
                arche_info.trigger_on_insert(entity, deferred.reborrow(), caller);

                world.flush();
            }
        }

        ::core::mem::forget(forget_guard);

        // -------------------------------------------------------------------
        // Return & Clear
        // -------------------------------------------------------------------

        // Map output
        for item in output.iter_mut() {
            *item = self.mapper.get_mapped(*item);
        }

        self.mapper.clear();
        self.cloned.clear();
        self.wait.clear();

        output
    }
}
