use voker_utils::vec::FastVec;

use crate::archetype::ArcheId;
use crate::bundle::{Bundle, BundleId};
use crate::component::{ComponentId, HookContext};
use crate::event::EntityComponentsTrigger;
use crate::utils::{DebugCheckedUnwrap, DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned};

impl EntityOwned<'_> {
    /// Remove the all components explicitly included in the Bundle.
    ///
    /// As same as [`EntityOwned::remove_explicit`].
    ///
    /// If some components do not exist, only existing components
    /// are removed; the program runs normally.
    ///
    /// # Panics
    ///
    /// Panics if this entity is despawned.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_ecs::prelude::*;
    ///
    /// #[derive(Default, Component, Clone)]
    /// struct Foo;
    ///
    /// #[derive(Component, Clone)]
    /// #[component(required = Foo)]
    /// struct Bar;
    ///
    /// let mut world = World::alloc();
    ///
    /// let mut entity = world.spawn(Bar);
    /// assert!(entity.contains::<Foo>());
    /// assert!(entity.contains::<Bar>());
    ///
    /// entity.remove::<Bar>();
    /// assert!(entity.contains::<Foo>()); // still exist
    /// assert!(!entity.contains::<Bar>());
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn remove<B: Bundle>(&mut self) -> &mut Self {
        self.remove_explicit_with_caller::<B>(DebugLocation::caller());
        self
    }

    /// Remove the all components explicitly included in the Bundle.
    ///
    /// If some components do not exist, only existing components
    /// are removed; the program runs normally.
    ///
    /// # Panics
    ///
    /// Panics if this entity is despawned.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_ecs::prelude::*;
    ///
    /// #[derive(Default, Component, Clone)]
    /// struct Foo;
    ///
    /// #[derive(Component, Clone)]
    /// #[component(required = Foo)]
    /// struct Bar;
    ///
    /// let mut world = World::alloc();
    ///
    /// let mut entity = world.spawn(Bar);
    /// assert!(entity.contains::<Foo>());
    /// assert!(entity.contains::<Bar>());
    ///
    /// entity.remove_explicit::<Bar>();
    /// assert!(entity.contains::<Foo>()); // still exist
    /// assert!(!entity.contains::<Bar>());
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn remove_explicit<B: Bundle>(&mut self) -> &mut Self {
        self.remove_explicit_with_caller::<B>(DebugLocation::caller());
        self
    }

    /// Remove the all components explicitly and implicitly
    /// included in the Bundle.
    ///
    /// If some components do not exist, only existing components
    /// are removed; the program runs normally.
    ///
    /// # Panics
    ///
    /// Panics if this entity is despawned.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_ecs::prelude::*;
    ///
    /// #[derive(Default, Component, Clone)]
    /// struct Foo;
    ///
    /// #[derive(Component, Clone)]
    /// #[component(required = Foo)]
    /// struct Bar;
    ///
    /// let mut world = World::alloc();
    ///
    /// let mut entity = world.spawn(Bar);
    /// assert!(entity.contains::<Foo>());
    /// assert!(entity.contains::<Bar>());
    ///
    /// entity.remove_required::<Bar>();
    /// assert!(!entity.contains::<Foo>()); // removed
    /// assert!(!entity.contains::<Bar>());
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn remove_required<B: Bundle>(&mut self) -> &mut Self {
        self.remove_required_with_caller::<B>(DebugLocation::caller());
        self
    }

    /// Remove the all components included in the target [`BundleInfo`].
    ///
    /// If some components do not exist, only existing components
    /// are removed; the program runs normally.
    ///
    /// [`BundleInfo`]: crate::bundle::BundleInfo
    ///
    /// # Panics
    ///
    /// Panics if this entity is despawned.
    ///  
    /// # Examples
    ///
    /// ```
    /// use core::any::TypeId;
    /// use voker_ecs::prelude::*;
    ///
    /// #[derive(Default, Component, Clone)]
    /// struct Foo;
    ///
    /// #[derive(Component, Clone)]
    /// #[component(required = Foo)]
    /// struct Bar;
    ///
    /// let mut world = World::alloc();
    ///
    /// let bundle_id = world.register_required_bundle::<Bar>();
    ///
    /// let mut entity = world.spawn(Bar);
    /// assert!(entity.contains::<Foo>());
    /// assert!(entity.contains::<Bar>());
    ///
    /// entity.remove_dynamic(bundle_id);
    /// assert!(!entity.contains::<Foo>());
    /// assert!(!entity.contains::<Bar>());
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn remove_dynamic(&mut self, bundle_id: BundleId) -> &mut Self {
        self.remove_dynamic_with_caller(bundle_id, DebugLocation::caller());
        self
    }

    /// Internal implementation of [`Self::remove_explicit`].
    #[inline]
    pub(crate) fn remove_explicit_with_caller<B: Bundle>(&mut self, caller: DebugLocation) {
        let world = unsafe { self.world.full_mut() };
        let bundle_id = world.register_explicit_bundle::<B>();

        self.remove_dynamic_with_caller(bundle_id, caller);
    }

    /// Internal implementation of [`Self::remove_required`].
    #[inline]
    pub(crate) fn remove_required_with_caller<B: Bundle>(&mut self, caller: DebugLocation) {
        let world = unsafe { self.world.full_mut() };
        let bundle_id = world.register_required_bundle::<B>();

        self.remove_dynamic_with_caller(bundle_id, caller);
    }

    /// Internal implementation of [`Self::remove_dynamic`].
    pub(crate) fn remove_dynamic_with_caller(
        &mut self,
        bundle_id: BundleId,
        caller: DebugLocation,
    ) {
        self.assert_is_spawned_with_caller(caller);

        let world = unsafe { self.world.full_mut() };

        let old_arche_id = unsafe { self.location.unwrap_unchecked().arche_id };
        let new_arche_id = world.arche_after_remove(old_arche_id, bundle_id);

        let guard = ForgetEntityOnPanic {
            entity: self.entity,
            world: self.world,
            caller,
        };

        if old_arche_id != new_arche_id {
            remove_moved(self, new_arche_id, caller);
        }

        ::core::mem::forget(guard);
    }
}

#[inline(never)]
fn remove_moved(this: &mut EntityOwned, new_arche_id: ArcheId, caller: DebugLocation) {
    let unsafe_world = this.world;
    let entity = this.entity;

    let mut location = unsafe { this.location.take().unwrap_unchecked() };

    let old_arche_id = location.arche_id;
    let [old_arche, new_arche] = unsafe {
        let arches = &mut unsafe_world.full_mut().archetypes;
        let indices = [old_arche_id.index(), new_arche_id.index()];
        arches.as_mut_slice().get_disjoint_unchecked_mut(indices)
    };

    debug_assert_eq!(old_arche.table_id(), location.table_id);

    {
        use crate::event::{DISCARD, Discard};

        // trigger_on_discard
        let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
        old_arche.on_discard_hooks().iter().for_each(|&(id, hook)| {
            if !new_arche.contains_component(id) {
                hook(world.reborrow(), HookContext { id, entity, caller });
            }
        });

        if old_arche.has_on_discard_observer() {
            let mut discard: FastVec<ComponentId, 4> = FastVec::new();
            let data = discard.data();

            old_arche.components().iter().for_each(|&id| {
                if !new_arche.contains_component(id) {
                    data.push(id);
                }
            });

            let mut event = Discard { entity };
            let mut trigger = EntityComponentsTrigger {
                components: data.as_slice(),
                old_archetype: Some(old_arche),
                new_archetype: Some(new_arche),
            };
            unsafe {
                world.trigger_raw(DISCARD, &mut event, &mut trigger, caller);
            }
        }
    }

    {
        use crate::event::{REMOVE, Remove};

        // trigger_on_remove
        let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
        old_arche.on_remove_hooks().iter().for_each(|&(id, hook)| {
            if !new_arche.contains_component(id) {
                hook(world.reborrow(), HookContext { id, entity, caller });
            }
        });

        if old_arche.has_on_remove_observer() {
            let mut discard: FastVec<ComponentId, 4> = FastVec::new();
            let data = discard.data();

            old_arche.components().iter().for_each(|&id| {
                if !new_arche.contains_component(id) {
                    data.push(id);
                }
            });

            let mut event = Remove { entity };
            let mut trigger = EntityComponentsTrigger {
                components: data.as_slice(),
                old_archetype: Some(old_arche),
                new_archetype: Some(new_arche),
            };
            unsafe {
                world.trigger_raw(REMOVE, &mut event, &mut trigger, caller);
            }
        }
    }

    {
        // Move Arche
        let new_arche_row = unsafe {
            let moved = old_arche.dealloc_row(location.arche_row);
            unsafe_world.full_mut().entities.update_row(moved).unwrap();
            new_arche.alloc_row(entity)
        };

        location.arche_id = new_arche_id;
        location.arche_row = new_arche_row;
    }

    {
        // Move Table
        let old_table_id = old_arche.table_id();
        let new_table_id = new_arche.table_id();

        if old_table_id != new_table_id {
            let table_row = location.table_row;
            let [old_table, new_table] = unsafe {
                let tables = &mut unsafe_world.full_mut().storages.tables;
                let indices = [old_table_id.index(), new_table_id.index()];
                tables.as_mut_slice().get_disjoint_unchecked_mut(indices)
            };
            let new_row = unsafe {
                let (moved, new) = old_table.move_row::<true>(table_row, new_table);
                unsafe_world.full_mut().entities.update_row(moved).unwrap();
                new
            };
            location.table_id = new_table_id;
            location.table_row = new_row;
        }
    }

    {
        // Move Map
        let world = unsafe { unsafe_world.full_mut() };
        let maps = &mut world.storages.maps;
        old_arche.sparse_components().iter().for_each(|&id| {
            if !new_arche.contains_sparse_component(id) {
                let map_id = unsafe { maps.get_id(id).debug_checked_unwrap() };
                let map = unsafe { maps.get_unchecked_mut(map_id) };
                let map_row = map.get_map_row(entity).unwrap();
                unsafe {
                    map.dealloc_row::<true>(map_row);
                }
            }
        });
    }

    unsafe {
        // update location
        let world = unsafe_world.full_mut();

        world.entities.update_location(entity, location).unwrap();

        world.flush();

        this.relocate();
    }
}
