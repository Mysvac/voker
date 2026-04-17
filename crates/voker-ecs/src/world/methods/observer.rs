use voker_utils::vec::SmallVec;

use crate::event::{Event, EventContext, EventId, Trigger};
use crate::observer::{IntoObserver, ObservedBy, Observer, ObserverId};
use crate::utils::DebugLocation;
use crate::world::World;

impl World {
    /// Registers a fully-built [`Observer`] into this world and returns its [`ObserverId`].
    #[inline(never)]
    pub(crate) fn register_observer(&mut self, observer: Observer) -> ObserverId {
        if let Some(flags) = observer.flags() {
            if observer.observed_components.is_empty() {
                for arche in self.archetypes.iter_mut() {
                    arche.merge_observer_flags(flags);
                }
            } else {
                let slice = observer.observed_components.as_slice();
                if voker_task::cfg::multi_threaded!() && self.archetypes.len() > 2400 {
                    use voker_task::ParallelSlice;
                    self.archetypes.as_mut_slice().par_each_mut(|arche| {
                        if slice.iter().any(|&id| arche.contains_component(id)) {
                            arche.merge_observer_flags(flags);
                        }
                    });
                } else {
                    for arche in self.archetypes.iter_mut() {
                        if slice.iter().any(|&id| arche.contains_component(id)) {
                            arche.merge_observer_flags(flags);
                        }
                    }
                }
            }
        }

        let entity_based = !observer.observed_entities.is_empty();

        let observer_id = self.observers.register(observer);

        if entity_based {
            let unsafe_world = self.unsafe_world();
            let observers = unsafe { &mut unsafe_world.data_mut().observers };
            let entity_world = unsafe { unsafe_world.full_mut() };
            let observer = unsafe { self.observers.get_unchecked(observer_id) };

            for entity in observer.observed_entities.clone() {
                let mut owned = match entity_world.get_entity_owned(entity) {
                    Ok(val) => val,
                    Err(e) => unsafe {
                        let observer = observers.get_unchecked_mut(observer_id);

                        observer.observed_entities.remove(&entity);
                        let name = observer.system_name();

                        log::warn!(
                            "Observer `{name}` try to observe a despawned entity, the target has been removed. {e}"
                        );

                        if observer.observed_entities.is_empty() {
                            observers.observers.remove(observer_id);
                            return observer_id;
                        }

                        continue;
                    },
                };

                if let Some(mut by) = owned.get_mut::<ObservedBy>() {
                    by.0.push(observer_id);
                } else {
                    let by = ObservedBy(SmallVec::from_slice(&[observer_id]));
                    owned.insert(by);
                }
            }
        }

        observer_id
    }

    /// Converts and registers an observer system, returning its [`ObserverId`].
    ///
    /// Events contained within will be automatically registered.
    pub fn add_observer<M>(&mut self, observer: impl IntoObserver<M>) -> ObserverId {
        let observer = observer.into_observer(self);
        self.register_observer(observer)
    }

    /// Triggers the given [`Event`], which will run any [`Observer`]s watching for it.
    ///
    /// For a variant that borrows the `event` rather than consuming it, use [`World::trigger_ref`] instead.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn trigger<'a, E: Event<Trigger<'a>: Default>>(&mut self, mut event: E) {
        let caller = DebugLocation::caller();
        let event_id = self.register_event::<E>();
        let mut trigger = <E::Trigger<'a> as Default>::default();
        unsafe {
            self.trigger_raw(event_id, &mut event, &mut trigger, caller);
        }
        self.flush();
    }

    /// Triggers the given [`Event`] using the given [`Trigger`](crate::event::Trigger), which will run any [`Observer`]s watching for it.
    ///
    /// For a variant that borrows the `event` rather than consuming it, use [`World::trigger_ref`] instead.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn trigger_with<'a, E: Event>(&mut self, mut event: E, mut trigger: E::Trigger<'a>) {
        let caller = DebugLocation::caller();
        let event_id = self.register_event::<E>();
        unsafe {
            self.trigger_raw(event_id, &mut event, &mut trigger, caller);
        }
        self.flush();
    }

    /// Triggers the given mutable [`Event`] reference, which will run any [`Observer`]s watching for it.
    ///
    /// Compared to [`World::trigger`], this method is most useful when it's necessary to check
    /// or use the event after it has been modified by observers.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn trigger_ref<'a, E: Event<Trigger<'a>: Default>>(&mut self, event: &mut E) {
        let caller = DebugLocation::caller();
        let event_id = self.register_event::<E>();
        let mut trigger = <E::Trigger<'a> as Default>::default();
        unsafe {
            self.trigger_raw(event_id, event, &mut trigger, caller);
        }
        self.flush();
    }

    /// Triggers an event using an explicit caller location.
    ///
    /// This is an internal helper used by command/deferred entry points that
    /// already captured a caller location.
    ///
    /// Like other high-level trigger helpers, this method flushes deferred
    /// commands after observer execution.
    pub(crate) fn trigger_with_caller<'a, E: Event>(
        &mut self,
        event: &mut E,
        trigger: &mut E::Trigger<'a>,
        caller: DebugLocation,
    ) {
        let event_id = self.register_event::<E>();
        unsafe {
            self.trigger_raw(event_id, event, trigger, caller);
        }
        self.flush();
    }

    /// Low-level trigger execution primitive without automatic flushing.
    ///
    /// This method looks up observer caches by `event_id` and invokes
    /// [`Trigger::trigger`] directly.
    ///
    /// Prefer higher-level trigger helpers unless you intentionally need to
    /// control flush boundaries yourself.
    ///
    /// # Safety
    ///
    /// Caller must ensure:
    /// - `event_id` corresponds to the concrete event type `E`.
    /// - `trigger` is valid for `E` and does not violate aliasing rules.
    /// - world aliasing guarantees are respected across deferred/observer access.
    pub(crate) unsafe fn trigger_raw<'a, E: Event>(
        &mut self,
        event_id: EventId,
        event: &mut E,
        trigger: &mut E::Trigger<'a>,
        caller: DebugLocation,
    ) {
        // SAFETY: You cannot get a mutable reference to `observers` from `DeferredWorld`
        let (world, observers) = unsafe {
            let world = self.unsafe_world();
            let observers = &world.data_mut().observers;
            let Some(observers) = observers.get_observers(event_id) else {
                return;
            };
            // SAFETY: The only outstanding reference to world is `observers`
            (world.deferred(), observers)
        };
        let context = EventContext {
            id: event_id,
            caller,
        };

        // SAFETY:
        // - `observers` comes from `world`, and corresponds to the `event_key`, as it was looked up above
        // - trigger_context contains the correct event_key for `event`, as enforced by the call to `trigger_raw`
        // - This method is being called for an `event` whose `Event::Trigger` matches, as the input trigger is E::Trigger.
        unsafe {
            trigger.trigger(world, context, observers, event);
        }
    }
}
