use core::fmt::Debug;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use crate::bundle::Bundle;
use crate::entity::Entity;
use crate::event::PropagateEntityTrigger;
use crate::event::{EntityEvent, Event, EventContext, EventId};
use crate::observer::ObserverId;
use crate::system::{IntoSystem, System, SystemInput};
use crate::traversal::Traversal;
use crate::utils::DebugLocation;

// -----------------------------------------------------------------------------
// On

pub struct On<'w, 't, E: Event, B: Bundle = ()> {
    observer: ObserverId,
    event_id: EventId,
    caller: DebugLocation,
    // SAFETY WARNING: never expose this 'w lifetime
    event: &'w mut E,
    // SAFETY WARNING: never expose this 'w lifetime
    trigger: &'w mut E::Trigger<'t>,
    // SAFETY WARNING: never expose this 'w lifetime
    _marker: PhantomData<B>,
}

impl<'w, 't, E: Event, B: Bundle> On<'w, 't, E, B> {
    pub fn new(
        event: &'w mut E,
        trigger: &'w mut E::Trigger<'t>,
        observer: ObserverId,
        context: EventContext,
    ) -> Self {
        Self {
            event,
            observer,
            trigger,
            caller: context.caller,
            event_id: context.id,
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> EventId {
        self.event_id
    }

    pub fn caller(&self) -> DebugLocation {
        self.caller
    }

    pub fn observer(&self) -> ObserverId {
        self.observer
    }

    pub fn event(&self) -> &E {
        self.event
    }

    pub fn event_mut(&mut self) -> &mut E {
        self.event
    }

    pub fn trigger(&self) -> &E::Trigger<'t> {
        self.trigger
    }

    pub fn trigger_mut(&mut self) -> &mut E::Trigger<'t> {
        self.trigger
    }
}

impl<'w, 't, const AUTO_PROPAGATE: bool, E, B, T> On<'w, 't, E, B>
where
    E: EntityEvent + for<'a> Event<Trigger<'a> = PropagateEntityTrigger<AUTO_PROPAGATE, E, T>>,
    B: Bundle,
    T: Traversal<E>,
{
    pub fn original_event_target(&self) -> Entity {
        self.trigger.original_event_target
    }

    pub fn set_propagate(&mut self, should_propagate: bool) {
        self.trigger.propagate = should_propagate;
    }

    pub fn get_propagate(&self) -> bool {
        self.trigger.propagate
    }
}

impl<'w, 't, E, B> Debug for On<'w, 't, E, B>
where
    B: Bundle,
    E: Event + Debug,
    for<'a> E::Trigger<'a>: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("On")
            .field("event", &self.event)
            .field("trigger", &self.trigger)
            .field("_marker", &self._marker)
            .finish()
    }
}

impl<'w, 't, E: Event, B: Bundle> Deref for On<'w, 't, E, B> {
    type Target = E;

    fn deref(&self) -> &Self::Target {
        self.event
    }
}

impl<'w, 't, E: Event, B: Bundle> DerefMut for On<'w, 't, E, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.event
    }
}

impl<E: Event, B: Bundle> SystemInput for On<'_, '_, E, B> {
    type Data<'i> = On<'i, 'i, E, B>;
    type Item<'i> = On<'i, 'i, E, B>;

    fn wrap(this: Self::Data<'_>) -> Self::Item<'_> {
        this
    }
}

// -----------------------------------------------------------------------------
// System

pub trait ObserverSystem<E: Event, B: Bundle, Output = ()>:
    System<Input = On<'static, 'static, E, B>, Output = Output> + Send + 'static
{
}

impl<E: Event, B: Bundle, Output, T> ObserverSystem<E, B, Output> for T where
    T: System<Input = On<'static, 'static, E, B>, Output = Output> + Send + 'static
{
}

pub trait IntoObserverSystem<E: Event, B: Bundle, M, Output = ()>: Send + 'static {
    type System: ObserverSystem<E, B, Output>;

    fn into_system(this: Self) -> Self::System;
}

impl<E: Event, B, M, Out, S> IntoObserverSystem<E, B, M, Out> for S
where
    S: IntoSystem<On<'static, 'static, E, B>, Out, M> + Send + 'static,
    S::System: ObserverSystem<E, B, Out>,
    E: 'static,
    B: Bundle,
{
    type System = S::System;

    fn into_system(this: Self) -> Self::System {
        IntoSystem::into_system(this)
    }
}
