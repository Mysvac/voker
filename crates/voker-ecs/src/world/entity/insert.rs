use voker_ptr::OwningPtr;

use crate::archetype::ArcheId;
use crate::bundle::Bundle;
use crate::component::ComponentWriter;
use crate::utils::{DebugLocation, ForgetEntityOnPanic};
use crate::world::EntityOwned;

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
    #[track_caller]
    pub fn insert<B: Bundle>(&mut self, bundle: B) {
        let world = unsafe { self.world.full_mut() };
        let bundle_id = world.register_required_bundle::<B>();
        let old_arche_id = self.location.arche_id;
        let new_arche_id = world.arche_after_insert(old_arche_id, bundle_id);

        let guard = ForgetEntityOnPanic {
            entity: self.entity,
            world: self.world,
            location: DebugLocation::caller(),
        };

        voker_ptr::into_owning!(bundle);

        if old_arche_id == new_arche_id {
            self.insert_local(bundle, B::write_explicit);
        } else {
            self.insert_moved(bundle, new_arche_id, B::write_explicit, B::write_required);
        }

        ::core::mem::forget(guard);
    }

    #[inline(never)]
    fn insert_local(
        &mut self,
        data: OwningPtr<'_>,
        write_explicit: unsafe fn(&mut ComponentWriter, usize),
    ) {
        let world = unsafe { self.world.data_mut() };

        let arche_id = self.location.arche_id;
        let arche = unsafe { world.archetypes.get_unchecked_mut(arche_id) };

        let table_id = arche.table_id();

        let table_row = self.location.table_row;
        let entity = self.entity;

        unsafe {
            let mut writer = ComponentWriter::new(self.world, data, entity, table_id, table_row);

            arche.components().iter().for_each(|&id| {
                writer.set_writed(id);
            });

            write_explicit(&mut writer, 0);
        }
    }

    #[inline(never)]
    fn insert_moved(
        &mut self,
        data: OwningPtr<'_>,
        new_arche_id: ArcheId,
        write_explicit: unsafe fn(&mut ComponentWriter, usize),
        write_required: unsafe fn(&mut ComponentWriter),
    ) {
        let old_arche_id = self.location.arche_id;
        let old_arche = unsafe { self.world.full_mut().archetypes.get_unchecked_mut(old_arche_id) };
        let new_arche = unsafe { self.world.full_mut().archetypes.get_unchecked_mut(new_arche_id) };
        assert_eq!(old_arche.table_id(), self.location.table_id);

        let moved = unsafe { old_arche.remove_entity(self.location.arche_row) };
        unsafe {
            self.world.full_mut().entities.update_row(moved).unwrap();
        }
        let new_arche_row = unsafe { new_arche.insert_entity(self.entity) };
        self.location.arche_id = new_arche_id;
        self.location.arche_row = new_arche_row;

        let old_table_id = old_arche.table_id();
        let new_table_id = new_arche.table_id();

        if old_table_id != new_table_id {
            let table_row = self.location.table_row;
            let old_table =
                unsafe { self.world.data_mut().storages.tables.get_unchecked_mut(old_table_id) };
            let new_table =
                unsafe { self.world.data_mut().storages.tables.get_unchecked_mut(new_table_id) };
            let (moved, new_row) =
                unsafe { old_table.move_to_and_forget_missing(table_row, new_table) };
            unsafe {
                self.world.full_mut().entities.update_row(moved).unwrap();
            }
            self.location.table_id = new_table_id;
            self.location.table_row = new_row;
        }

        let table_row = self.location.table_row;
        let table_id = self.location.table_id;
        let entity = self.entity;
        let world = unsafe { self.world.data_mut() };
        let old_arche = unsafe { world.archetypes.get_unchecked(old_arche_id) };

        unsafe {
            let mut writer = ComponentWriter::new(self.world, data, entity, table_id, table_row);

            old_arche.components().iter().for_each(|&id| {
                writer.set_writed(id);
            });

            write_explicit(&mut writer, 0);
            write_required(&mut writer);
        }

        unsafe {
            world.entities.update_location(self.entity, self.location).unwrap();
        }
    }
}
