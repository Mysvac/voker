use core::fmt::Debug;
use core::hash::Hash;
use core::ops::Deref;

use voker_utils::vec::SmallVec;

use crate::prelude::{Component, HookContext};
use crate::world::DeferredWorld;

// -----------------------------------------------------------------------------
// ObserverId

slotmap::new_key_type! {
    pub struct ObserverId;
}

// -----------------------------------------------------------------------------
// ObservedBy

#[derive(Component, Default, Debug, Clone)]
#[component(storage = "sparse", on_clone, on_discard, on_remove)]
pub struct ObservedBy(pub(crate) SmallVec<ObserverId, 4>);

impl Deref for ObservedBy {
    type Target = [ObserverId];

    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

impl ObservedBy {
    pub fn get(&self) -> &[ObserverId] {
        self.0.as_slice()
    }

    fn on_discard(world: DeferredWorld, ctx: HookContext) {
        let entity = ctx.entity;
        let observer_by = world.get::<Self>(entity).expect("should exist");
        let idents = observer_by.0.as_slice();

        for &id in idents {
            let world1 = unsafe { world.unsafe_world().data_mut() };
            let world2 = unsafe { world.unsafe_world().data_mut() };

            let observer = unsafe { world1.observers.get_unchecked_mut(id) };
            observer.observed_entities.remove(&entity);

            let event_id = observer.event_id;
            let observers = unsafe { world2.observers.runners.get_unchecked_mut(event_id.index()) };

            if let Some(maps) = observers.entity_observers.get_mut(&entity) {
                maps.remove(&id);
            }

            for cid in observer.observed_components.as_slice() {
                if let Some(maps) = observers.component_observers.get_mut(cid)
                    && let Some(maps) = maps.entity_component_observers.get_mut(&entity)
                {
                    maps.remove(&id);
                }
            }
        }
    }

    fn on_remove(world: DeferredWorld, ctx: HookContext) {
        let entity = ctx.entity;
        let observer_by = world.get::<Self>(entity).expect("should exist");
        let idents = observer_by.0.as_slice();

        for &id in idents {
            let world1 = unsafe { world.unsafe_world().data_mut() };
            let world2 = unsafe { world.unsafe_world().data_mut() };

            let observer = unsafe { world1.observers.get_unchecked_mut(id) };

            let event_id = observer.event_id;
            let observers = unsafe { world2.observers.runners.get_unchecked_mut(event_id.index()) };

            observers.entity_observers.remove(&entity);

            for cid in observer.observed_components.as_slice() {
                if let Some(maps) = observers.component_observers.get_mut(cid) {
                    maps.entity_component_observers.remove(&entity);
                }
            }

            if observer.observed_entities.is_empty() {
                world1.observers.observers.remove(id);
            }
        }
    }

    fn on_clone(world: DeferredWorld, ctx: HookContext) {
        let entity = ctx.entity;
        let observer_by = world.get::<Self>(entity).expect("should exist");
        let idents = observer_by.0.as_slice();

        for &id in idents {
            let world1 = unsafe { world.unsafe_world().data_mut() };
            let world2 = unsafe { world.unsafe_world().data_mut() };

            let observer = unsafe { world1.observers.get_unchecked_mut(id) };
            observer.observed_entities.insert(entity);

            let event_id = observer.event_id;
            let observers = unsafe { world2.observers.runners.get_unchecked_mut(event_id.index()) };

            observers
                .entity_observers
                .entry(entity)
                .or_default()
                .insert(id, observer.runner);

            for &cid in observer.observed_components.as_slice() {
                observers
                    .component_observers
                    .entry(cid)
                    .or_default()
                    .entity_component_observers
                    .entry(entity)
                    .or_default()
                    .insert(id, observer.runner);
            }
        }
    }
}
