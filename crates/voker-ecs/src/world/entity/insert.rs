use voker_ptr::OwningPtr;

use crate::archetype::ArcheId;
use crate::bundle::{Bundle, BundleId};
use crate::component::{ComponentWriter, HookContext};
use crate::link::LinkHookMode;
use crate::utils::{DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned};

impl EntityOwned<'_> {
    /// Insert components from given bundle.
    ///
    /// Required bundles will be automatically inserted.
    ///
    /// # Rules
    ///
    /// Explicit Components will overwrite the old components.
    ///
    /// Required Components will be automatically inserted **if not exits**.
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
    /// let mut entity = world.spawn(());
    ///
    /// assert!(!entity.contains::<Foo>());
    /// assert!(!entity.contains::<Bar>());
    ///
    /// entity.insert(Bar);
    ///
    /// assert!(entity.contains::<Foo>());
    /// assert!(entity.contains::<Bar>());
    /// ```
    #[inline]
    #[track_caller]
    pub fn insert<B: Bundle>(&mut self, bundle: B) {
        self.insert_with_caller(bundle, LinkHookMode::Run, DebugLocation::caller());
    }

    /// Inserts a new `Bundle` if the entity is missing **any** of its components.
    ///
    /// This method checks whether the entity already contains all component types defined in `B`.
    /// - If **all components already exist**, nothing happens.
    /// - If **at least one component is missing**, the provided closure `f` is called to generate
    ///   a new `B` instance, and **all components** from that `Bundle` are inserted into the entity,
    ///   **overwriting** any existing components of the same type.
    ///
    /// # Note
    /// To avoid accidental overwrites, **it is strongly recommended that `Bundle` contains only
    /// a single component type**, which eliminates the risk of false-positive overwrites.
    #[inline]
    #[track_caller]
    pub fn insert_if_new<B: Bundle>(&mut self, f: impl FnOnce() -> B) {
        self.insert_if_new_with_caller(f, LinkHookMode::Run, DebugLocation::caller());
    }

    #[inline]
    pub(crate) fn insert_if_new_with_caller<B: Bundle>(
        &mut self,
        f: impl FnOnce() -> B,
        link_hook_mode: LinkHookMode,
        caller: DebugLocation,
    ) {
        self.assert_is_spawned_with_caller(caller);

        let world = unsafe { self.world.full_mut() };
        let explicit_bundle_id = world.register_explicit_bundle::<B>();
        let old_arche_id = unsafe { self.location.unwrap_unchecked().arche_id };
        let arche_info = unsafe { world.archetypes.get_unchecked(old_arche_id) };
        let bundle_info = unsafe { world.bundles.get_unchecked(explicit_bundle_id) };
        let explicit_components = bundle_info.components();
        if explicit_components
            .iter()
            .all(|&id| arche_info.contains_component(id))
        {
            return;
        }

        self.insert_with_caller(f(), link_hook_mode, caller);
    }

    #[inline]
    pub(crate) fn insert_with_caller<B: Bundle>(
        &mut self,
        bundle: B,
        link_hook_mode: LinkHookMode,
        caller: DebugLocation,
    ) {
        self.assert_is_spawned_with_caller(caller);

        let world = unsafe { self.world.full_mut() };
        let required_bundle_id = world.register_required_bundle::<B>();
        let explicit_bundle_id = world.register_explicit_bundle::<B>();

        let old_arche_id = unsafe { self.location.unwrap_unchecked().arche_id };
        let new_arche_id = world.arche_after_insert(old_arche_id, required_bundle_id);

        let clear_guard = ForgetEntityOnPanic {
            entity: self.entity,
            world: self.world,
            location: caller,
        };

        voker_ptr::into_owning!(bundle);

        if old_arche_id == new_arche_id {
            insert_local(
                self,
                bundle,
                explicit_bundle_id,
                B::write_explicit,
                link_hook_mode,
                caller,
            );
        } else {
            insert_moved(
                self,
                bundle,
                new_arche_id,
                explicit_bundle_id,
                B::write_explicit,
                B::write_required,
                link_hook_mode,
                caller,
            );
        }

        ::core::mem::forget(clear_guard);
    }
}

#[inline(never)]
fn insert_local(
    this: &mut EntityOwned,
    data: OwningPtr<'_>,
    explicit_bundle_id: BundleId,
    write_explicit: unsafe fn(&mut ComponentWriter, usize),
    link_hook_mode: LinkHookMode,
    caller: DebugLocation,
) {
    let entity = this.entity;
    let unsafe_world = this.world;
    let world = unsafe { unsafe_world.data_mut() };

    // Take it to ensure the safety in panic.
    let location = unsafe { this.location.take().unwrap_unchecked() };

    let arche_id = location.arche_id;
    let arche = unsafe { world.archetypes.get_unchecked(arche_id) };
    let table_id = arche.table_id();
    let table_row = location.table_row;
    let bundle = unsafe { world.bundles.get_unchecked(explicit_bundle_id) };

    {
        // trigger_on_discard
        let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
        arche.on_discard_hooks().iter().for_each(|&(id, hook)| {
            if bundle.contains_component(id) {
                hook(
                    world.reborrow(),
                    HookContext {
                        id,
                        entity,
                        caller,
                        link_hook_mode,
                    },
                );
            }
        });
    }

    unsafe {
        // write date
        let mut writer = ComponentWriter::new(unsafe_world, data, entity, table_id, table_row);

        arche.components().iter().for_each(|&id| {
            writer.set_writed(id);
        });

        write_explicit(&mut writer, 0);
    }

    {
        // trigger_on_insert
        let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
        arche.on_insert_hooks().iter().for_each(|&(id, hook)| {
            if bundle.contains_component(id) {
                hook(
                    world.reborrow(),
                    HookContext {
                        id,
                        entity,
                        caller,
                        link_hook_mode,
                    },
                );
            }
        });
    }

    world.flush();

    // Reset EntityLocation.
    this.relocate();
}

#[inline(never)]
fn insert_moved(
    this: &mut EntityOwned,
    data: OwningPtr<'_>,
    new_arche_id: ArcheId,
    explicit_bundle_id: BundleId,
    write_explicit: unsafe fn(&mut ComponentWriter, usize),
    write_required: unsafe fn(&mut ComponentWriter),
    link_hook_mode: LinkHookMode,
    caller: DebugLocation,
) {
    let entity = this.entity;
    let unsafe_world = this.world;
    let world = unsafe { unsafe_world.data_mut() };

    // SAFETY: Already checked in `insert_with_caller`.
    // Take it to ensure the safety in panic.
    let mut location = unsafe { this.location.take().unwrap_unchecked() };

    let old_arche_id = location.arche_id;
    let [old_arche, new_arche] = unsafe {
        let arches = &mut unsafe_world.full_mut().archetypes;
        let indices = [old_arche_id.index(), new_arche_id.index()];
        arches.as_mut_slice().get_disjoint_unchecked_mut(indices)
    };

    debug_assert_eq!(old_arche.table_id(), location.table_id);

    let bundle = unsafe { world.bundles.get_unchecked(explicit_bundle_id) };

    {
        // trigger_on_discard
        let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
        old_arche.on_discard_hooks().iter().for_each(|&(id, hook)| {
            if bundle.contains_component(id) {
                hook(
                    world.reborrow(),
                    HookContext {
                        id,
                        entity,
                        caller,
                        link_hook_mode,
                    },
                );
            }
        });
    }

    {
        // Move Arche
        let new_arche_row = unsafe {
            let moved = old_arche.remove_entity(location.arche_row);
            world.entities.update_row(moved).unwrap();
            new_arche.insert_entity(entity)
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
                let (moved, new) = old_table.move_row::<false>(table_row, new_table);
                unsafe_world.full_mut().entities.update_row(moved).unwrap();
                new
            };
            location.table_id = new_table_id;
            location.table_row = new_row;
        }
    }

    unsafe {
        // Write data
        let table_row = location.table_row;
        let table_id = location.table_id;

        let mut writer = ComponentWriter::new(unsafe_world, data, entity, table_id, table_row);

        old_arche.components().iter().for_each(|&id| {
            writer.set_writed(id);
        });

        write_explicit(&mut writer, 0);
        write_required(&mut writer);

        world.entities.update_location(entity, location).unwrap();
    }

    {
        // trigger_on_add
        let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
        new_arche.on_add_hooks().iter().for_each(|&(id, hook)| {
            if !old_arche.contains_component(id) {
                hook(
                    world.reborrow(),
                    HookContext {
                        id,
                        entity,
                        caller,
                        link_hook_mode,
                    },
                );
            }
        });
    }

    {
        // trigger_on_insert
        let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
        new_arche.on_insert_hooks().iter().for_each(|&(id, hook)| {
            if !old_arche.contains_component(id) || bundle.contains_component(id) {
                hook(
                    world.reborrow(),
                    HookContext {
                        id,
                        entity,
                        caller,
                        link_hook_mode,
                    },
                );
            }
        });
    }

    world.flush();

    // Reset EntityLocation.
    this.relocate();
}
