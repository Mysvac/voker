use voker_ptr::OwningPtr;

use crate::archetype::ArcheId;
use crate::bundle::{Bundle, BundleId};
use crate::component::{ComponentHookContext, ComponentWriter};
use crate::utils::{DebugLocation, ForgetEntityOnPanic};
use crate::world::{DeferredWorld, EntityOwned};

impl EntityOwned<'_> {
    /// Insert component.
    ///
    /// # Rules
    ///
    /// Existing components will be overwritten.
    ///
    /// If need required components will be create,
    /// but will not overwrite existing components.
    ///
    /// # Examples
    ///
    /// ```
    /// # use voker_ecs::world::World;
    /// # use voker_ecs::component::Component;
    /// # #[derive(Component, Debug)]
    /// # struct Foo;
    /// # #[derive(Component, Debug)]
    /// # struct Bar;
    /// let mut world = World::alloc();
    ///
    /// let mut entity = world.spawn(Foo);
    /// assert!(entity.contains::<Foo>());
    /// assert!(!entity.contains::<Bar>());
    ///
    /// entity.insert(Bar);
    /// assert!(entity.contains::<Bar>());
    /// ```
    #[inline]
    #[track_caller]
    pub fn insert<B: Bundle>(&mut self, bundle: B) {
        self.insert_with_caller(bundle, DebugLocation::caller());
    }

    #[inline]
    pub(crate) fn insert_with_caller<B: Bundle>(&mut self, bundle: B, caller: DebugLocation) {
        let world = unsafe { self.world.full_mut() };

        let required_bundle_id = world.register_required_bundle::<B>();
        let explicit_bundle_id = world.register_explicit_bundle::<B>();

        let old_arche_id = self.location.arche_id;
        let new_arche_id = world.arche_after_insert(old_arche_id, required_bundle_id);

        let guard = ForgetEntityOnPanic {
            entity: self.entity,
            world: self.world,
            location: caller,
        };

        voker_ptr::into_owning!(bundle);

        if old_arche_id == new_arche_id {
            self.insert_local(bundle, explicit_bundle_id, B::write_explicit, caller);
        } else {
            self.insert_moved(
                bundle,
                new_arche_id,
                explicit_bundle_id,
                B::write_explicit,
                B::write_required,
                caller,
            );
        }

        ::core::mem::forget(guard);
    }

    #[inline(never)]
    fn insert_local(
        &mut self,
        data: OwningPtr<'_>,
        explicit_bundle_id: BundleId,
        write_explicit: unsafe fn(&mut ComponentWriter, usize),
        caller: DebugLocation,
    ) {
        let entity = self.entity;
        let unsafe_world = self.world;
        let world = unsafe { unsafe_world.data_mut() };

        let arche_id = self.location.arche_id;
        let arche = unsafe { world.archetypes.get_unchecked(arche_id) };
        let table_id = arche.table_id();
        let table_row = self.location.table_row;

        let bundle = unsafe { world.bundles.get_unchecked(explicit_bundle_id) };

        {
            // trigger_on_discard
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            arche.discard_hooks().iter().for_each(|&(id, hook)| {
                if bundle.contains_component(id) {
                    hook(
                        world.reborrow(),
                        ComponentHookContext { id, entity, caller },
                    );
                }
            });
        }

        unsafe {
            // write date
            let mut writer = ComponentWriter::new(self.world, data, entity, table_id, table_row);

            arche.components().iter().for_each(|&id| {
                writer.set_writed(id);
            });

            write_explicit(&mut writer, 0);
        }

        {
            // trigger_on_insert
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            let entity = self.entity;
            arche.insert_hooks().iter().for_each(|&(id, hook)| {
                if bundle.contains_component(id) {
                    hook(
                        world.reborrow(),
                        ComponentHookContext { id, entity, caller },
                    );
                }
            });
        }

        world.flush();
    }

    #[inline(never)]
    fn insert_moved(
        &mut self,
        data: OwningPtr<'_>,
        new_arche_id: ArcheId,
        explicit_bundle_id: BundleId,
        write_explicit: unsafe fn(&mut ComponentWriter, usize),
        write_required: unsafe fn(&mut ComponentWriter),
        caller: DebugLocation,
    ) {
        let entity = self.entity;
        let unsafe_world = self.world;
        let world = unsafe { unsafe_world.data_mut() };

        let old_arche_id = self.location.arche_id;
        let old_arche =
            unsafe { unsafe_world.full_mut().archetypes.get_unchecked_mut(old_arche_id) };
        let new_arche =
            unsafe { unsafe_world.full_mut().archetypes.get_unchecked_mut(new_arche_id) };
        debug_assert_eq!(old_arche.table_id(), self.location.table_id);
        let bundle = unsafe { world.bundles.get_unchecked(explicit_bundle_id) };

        {
            // trigger_on_discard
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            old_arche.discard_hooks().iter().for_each(|&(id, hook)| {
                if bundle.contains_component(id) {
                    hook(
                        world.reborrow(),
                        ComponentHookContext { id, entity, caller },
                    );
                }
            });
        }

        // Move Arche
        let new_arche_row = unsafe {
            let moved = old_arche.remove_entity(self.location.arche_row);
            world.entities.update_row(moved).unwrap();
            new_arche.insert_entity(self.entity)
        };

        self.location.arche_id = new_arche_id;
        self.location.arche_row = new_arche_row;

        let old_table_id = old_arche.table_id();
        let new_table_id = new_arche.table_id();

        // Move Table
        if old_table_id != new_table_id {
            let table_row = self.location.table_row;
            let old_table = unsafe {
                unsafe_world
                    .full_mut()
                    .storages
                    .tables
                    .get_unchecked_mut(old_table_id)
            };
            let new_table = unsafe {
                unsafe_world
                    .full_mut()
                    .storages
                    .tables
                    .get_unchecked_mut(new_table_id)
            };
            let (moved, new_table_row) =
                unsafe { old_table.move_to_and_forget_missing(table_row, new_table) };
            unsafe {
                world.entities.update_row(moved).unwrap();
            }
            self.location.table_id = new_table_id;
            self.location.table_row = new_table_row;
        }

        let table_row = self.location.table_row;
        let table_id = self.location.table_id;
        let old_arche = unsafe { world.archetypes.get_unchecked(old_arche_id) };

        unsafe {
            // Write data
            let mut writer = ComponentWriter::new(self.world, data, entity, table_id, table_row);

            old_arche.components().iter().for_each(|&id| {
                writer.set_writed(id);
            });

            write_explicit(&mut writer, 0);
            write_required(&mut writer);

            world.entities.update_location(self.entity, self.location).unwrap();
        }

        {
            // trigger_on_add
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            new_arche.add_hooks().iter().for_each(|&(id, hook)| {
                if !old_arche.contains_component(id) {
                    hook(
                        world.reborrow(),
                        ComponentHookContext { id, entity, caller },
                    );
                }
            });
        }

        {
            // trigger_on_insert
            let mut world: DeferredWorld = unsafe { unsafe_world.deferred() };
            new_arche.insert_hooks().iter().for_each(|&(id, hook)| {
                if !old_arche.contains_component(id) || bundle.contains_component(id) {
                    hook(
                        world.reborrow(),
                        ComponentHookContext { id, entity, caller },
                    );
                }
            });
        }

        world.flush();
    }
}
