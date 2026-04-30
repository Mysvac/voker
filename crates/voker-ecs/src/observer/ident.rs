use core::fmt::Debug;
use core::hash::Hash;
use core::ops::Deref;

use voker_utils::vec::SmallVec;

use crate::prelude::{Component, HookContext};
use crate::world::DeferredWorld;

// -----------------------------------------------------------------------------
// ObserverId

slotmap::new_key_type! {
    /// Stable identifier for an observer stored in [`Observers`](crate::observer::Observers).
    pub struct ObserverId;
}

// -----------------------------------------------------------------------------
// ObservedBy

/// Component storing observer ids currently watching this entity.
///
/// This component is maintained internally by observer registration and
/// lifecycle hooks.
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
    /// Returns the observer ids currently associated with this entity.
    pub fn get(&self) -> &[ObserverId] {
        self.0.as_slice()
    }

    fn on_discard(world: DeferredWorld, ctx: HookContext) {
        let entity = ctx.entity;
        let observer_by = world.get::<Self>(entity).expect("should exist");
        let idents = observer_by.0.as_slice();

        let world = unsafe { world.unsafe_world().data_mut() };
        let observers = &mut world.observers;

        for &id in idents {
            let observer = unsafe { observers.observers.get_unchecked_mut(id) };
            observer.entities.remove(&entity);
            let event_id = observer.event_id;
            let cached = unsafe { observers.runners.get_unchecked_mut(event_id.index()) };

            if let Some(maps) = cached.entity_observers.get_mut(&entity) {
                maps.remove(&id);
            }

            for cid in observer.components.as_slice() {
                if let Some(maps) = cached.component_observers.get_mut(cid)
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

        let world = unsafe { world.unsafe_world().data_mut() };
        let observers = &mut world.observers;

        for &id in idents {
            let observer = unsafe { observers.observers.get_unchecked_mut(id) };
            let event_id = observer.event_id;
            let cached = unsafe { observers.runners.get_unchecked_mut(event_id.index()) };

            cached.entity_observers.remove(&entity);

            for cid in observer.components.as_slice() {
                if let Some(maps) = cached.component_observers.get_mut(cid) {
                    maps.entity_component_observers.remove(&entity);
                }
            }

            if observer.entities.is_empty() {
                observers.observers.remove(id);
            }
        }
    }

    fn on_clone(world: DeferredWorld, ctx: HookContext) {
        let entity = ctx.entity;
        let observer_by = world.get::<Self>(entity).expect("should exist");
        let idents = observer_by.0.as_slice();

        let world = unsafe { world.unsafe_world().data_mut() };
        let observers = &mut world.observers;

        for &id in idents {
            let observer = unsafe { observers.observers.get_unchecked_mut(id) };
            observer.entities.insert(entity);
            let event_id = observer.event_id;
            let cached = unsafe { observers.runners.get_unchecked_mut(event_id.index()) };

            cached
                .entity_observers
                .entry(entity)
                .or_default()
                .insert(id, observer.runner);

            for &cid in observer.components.as_slice() {
                cached
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
