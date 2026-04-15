use voker_ptr::PtrMut;

use core::fmt::Debug;
use core::marker::PhantomData;

use super::{EntityEvent, EntityEventMut, Event, EventContext};
use crate::archetype::Archetype;
use crate::entity::Entity;
use crate::observer::CachedObservers;
use crate::prelude::ComponentId;
use crate::traversal::Traversal;
use crate::world::DeferredWorld;

// -----------------------------------------------------------------------------
// Trigger

pub unsafe trait Trigger<E: Event> {
    unsafe fn trigger(
        &mut self,
        world: DeferredWorld,
        context: EventContext,
        observers: &CachedObservers,
        event: &mut E,
    );
}

// -----------------------------------------------------------------------------
// GlobalTrigger

#[derive(Default, Debug)]
pub struct GlobalTrigger;

unsafe impl<E: for<'a> Event<Trigger<'a> = Self>> Trigger<E> for GlobalTrigger {
    unsafe fn trigger(
        &mut self,
        world: DeferredWorld,
        context: EventContext,
        observers: &CachedObservers,
        event: &mut E,
    ) {
        let event_ptr = PtrMut::from_mut(event);
        let trigger_ptr = PtrMut::from_mut(self);

        unsafe {
            Self::trigger_internal(world, context, observers, event_ptr, trigger_ptr);
        }
    }
}

impl GlobalTrigger {
    #[inline(never)]
    unsafe fn trigger_internal(
        mut world: DeferredWorld,
        context: EventContext,
        observers: &CachedObservers,
        mut event: PtrMut,
        mut trigger: PtrMut,
    ) {
        unsafe {
            let world = world.unsafe_world().full_mut();
            world.last_trigger = world.last_trigger.wrapping_add(1);
        }

        for (observer, runner) in observers.global_observers() {
            unsafe {
                (runner)(
                    world.reborrow(),
                    context,
                    *observer,
                    event.reborrow(),
                    trigger.reborrow(),
                );
            }
        }
    }
}

// -----------------------------------------------------------------------------
// EntityTrigger

#[derive(Default, Debug)]
pub struct EntityTrigger;

unsafe impl<E: EntityEvent + for<'a> Event<Trigger<'a> = Self>> Trigger<E> for EntityTrigger {
    unsafe fn trigger(
        &mut self,
        world: DeferredWorld,
        context: EventContext,
        observers: &CachedObservers,
        event: &mut E,
    ) {
        let entity = event.event_target();
        let event_ptr = PtrMut::from_mut(event);
        let trigger_ptr = PtrMut::from_mut(self);
        unsafe {
            Self::trigger_internal(world, context, observers, event_ptr, trigger_ptr, entity);
        }
    }
}

impl EntityTrigger {
    #[inline(never)]
    unsafe fn trigger_internal(
        mut world: DeferredWorld,
        context: EventContext,
        observers: &CachedObservers,
        mut event: PtrMut,
        mut trigger: PtrMut,
        target_entity: Entity,
    ) {
        unsafe {
            let world = world.unsafe_world().full_mut();
            world.last_trigger = world.last_trigger.wrapping_add(1);
        }

        for (observer, runner) in observers.global_observers() {
            unsafe {
                (runner)(
                    world.reborrow(),
                    context,
                    *observer,
                    event.reborrow(),
                    trigger.reborrow(),
                );
            }
        }

        if let Some(map) = observers.entity_observers().get(&target_entity) {
            for (observer, runner) in map {
                unsafe {
                    (runner)(
                        world.reborrow(),
                        context,
                        *observer,
                        event.reborrow(),
                        trigger.reborrow(),
                    );
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// EntityComponentsTrigger

#[derive(Default)]
pub struct EntityComponentsTrigger<'a> {
    pub components: &'a [ComponentId],
    pub old_archetype: Option<&'a Archetype>,
    pub new_archetype: Option<&'a Archetype>,
}

unsafe impl<'a, E> Trigger<E> for EntityComponentsTrigger<'a>
where
    E: EntityEvent + Event<Trigger<'a> = EntityComponentsTrigger<'a>>,
{
    unsafe fn trigger(
        &mut self,
        world: DeferredWorld,
        context: EventContext,
        observers: &CachedObservers,
        event: &mut E,
    ) {
        let entity = event.event_target();
        unsafe {
            self.trigger_internal(world, context, observers, event.into(), entity);
        }
    }
}

impl<'a> EntityComponentsTrigger<'a> {
    #[inline(never)]
    unsafe fn trigger_internal(
        &mut self,
        mut world: DeferredWorld,
        context: EventContext,
        observers: &CachedObservers,
        mut event: PtrMut,
        entity: Entity,
    ) {
        let components = self.components;
        let mut trigger = PtrMut::from_mut(self);

        {
            unsafe {
                EntityTrigger::trigger_internal(
                    world.reborrow(),
                    context,
                    observers,
                    event.reborrow(),
                    trigger.reborrow(),
                    entity,
                );
            }
        }

        for id in components {
            if let Some(component_observers) = observers.component_observers().get(id) {
                for (observer, runner) in component_observers.global_observers() {
                    unsafe {
                        (runner)(
                            world.reborrow(),
                            context,
                            *observer,
                            event.reborrow(),
                            trigger.reborrow(),
                        );
                    }
                }

                if let Some(map) = component_observers.entity_component_observers().get(&entity) {
                    for (observer, runner) in map {
                        unsafe {
                            (runner)(
                                world.reborrow(),
                                context,
                                *observer,
                                event.reborrow(),
                                trigger.reborrow(),
                            );
                        }
                    }
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// PropagateEntityTrigger

pub struct PropagateEntityTrigger<const AUTO_PROPAGATE: bool, E: EntityEvent, T: Traversal<E>> {
    /// The original [`Entity`] the [`Event`] was _first_ triggered for.
    pub original_event_target: Entity,

    /// Whether or not to continue propagating using the `T` [`Traversal`]. If this is false,
    /// The [`Traversal`] will stop on the current entity.
    pub propagate: bool,

    _marker: PhantomData<(E, T)>,
}

impl<const AUTO_PROPAGATE: bool, E, T> Default for PropagateEntityTrigger<AUTO_PROPAGATE, E, T>
where
    E: EntityEvent,
    T: Traversal<E>,
{
    fn default() -> Self {
        Self {
            original_event_target: Entity::PLACEHOLDER,
            propagate: AUTO_PROPAGATE,
            _marker: Default::default(),
        }
    }
}

impl<const AUTO_PROPAGATE: bool, E, T> Debug for PropagateEntityTrigger<AUTO_PROPAGATE, E, T>
where
    E: EntityEvent,
    T: Traversal<E>,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PropagateEntityTrigger")
            .field("original_event_target", &self.original_event_target)
            .field("propagate", &self.propagate)
            .field("_marker", &self._marker)
            .finish()
    }
}

unsafe impl<const AUTO_PROPAGATE: bool, E, T> Trigger<E>
    for PropagateEntityTrigger<AUTO_PROPAGATE, E, T>
where
    E: EntityEvent + EntityEventMut + for<'a> Event<Trigger<'a> = Self>,
    T: Traversal<E>,
{
    unsafe fn trigger(
        &mut self,
        mut world: DeferredWorld,
        context: EventContext,
        observers: &CachedObservers,
        event: &mut E,
    ) {
        let mut current_entity = event.event_target();
        self.original_event_target = current_entity;

        {
            let event_ptr = PtrMut::from_mut(event);
            let trigger_ptr = PtrMut::from_mut(self);

            unsafe {
                EntityTrigger::trigger_internal(
                    world.reborrow(),
                    context,
                    observers,
                    event_ptr,
                    trigger_ptr,
                    current_entity,
                );
            }
        }

        loop {
            if !self.propagate {
                return;
            }

            if let Ok(entity) = world.get_entity_ref(current_entity)
                && let Some(traverse_to) = T::traverse(entity, event)
            {
                current_entity = traverse_to;
            } else {
                break;
            }

            event.set_event_target(current_entity);
            let event_ptr = PtrMut::from_mut(event);
            let trigger_ptr = PtrMut::from_mut(self);

            unsafe {
                EntityTrigger::trigger_internal(
                    world.reborrow(),
                    context,
                    observers,
                    event_ptr,
                    trigger_ptr,
                    current_entity,
                );
            }
        }
    }
}
