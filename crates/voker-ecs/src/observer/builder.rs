use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;

use voker_ptr::PtrMut;
use voker_utils::hash::SparseHashSet;

use crate::bundle::Bundle;
use crate::entity::Entity;
use crate::error::ErrorContext;
use crate::event::{Event, EventContext};
use crate::observer::{IntoObserverSystem, Observer, ObserverId, ObserverSystem, On};
use crate::system::System;
use crate::utils::DebugCheckedUnwrap;
use crate::world::{DeferredWorld, World};

impl Observer {
    #[track_caller]
    pub(crate) fn build<E, B, M, S>(world: &mut World, system: S) -> Observer
    where
        E: Event,
        B: Bundle,
        S: IntoObserverSystem<E, B, M>,
    {
        let mut system = Box::new(IntoObserverSystem::into_system(system));

        assert!(
            !system.is_exclusive(),
            "Exclusive system `{}` can not be used as observer.\n\
            Instead of `&mut World`, use either `DeferredWorld` if\
            you do not need structural changes, or `Commands` if you do.",
            system.id().name()
        );

        let _ = system.initialize(world);
        let bundle_id = world.register_explicit_bundle::<B>();
        let idents = unsafe { world.bundles.get_unchecked(bundle_id).components() };

        Observer {
            event_id: world.register_event::<E>(),
            last_trigger: world.last_trigger(),
            system,
            runner: observer_system_runner::<E, B, S::System>,
            error_handler: None,
            observed_components: Vec::from(idents),
            observed_entities: SparseHashSet::new(),
        }
    }
}

// -----------------------------------------------------------------------------
// IntoObserver

pub trait IntoObserver<Marker>: Send + 'static {
    fn into_observer(self, world: &mut World) -> Observer;
}

impl IntoObserver<()> for Observer {
    fn into_observer(self, _: &mut World) -> Observer {
        self
    }
}

impl<E: Event, B: Bundle, M, T: IntoObserverSystem<E, B, M>> IntoObserver<(E, B, M)> for T {
    fn into_observer(self, world: &mut World) -> Observer {
        Observer::build(world, self)
    }
}

pub trait IntoEntityObserver<Marker>: Send + 'static {
    fn into_observer_for_entity(self, entity: Entity, world: &mut World) -> Observer;
}

impl<M, T: IntoObserver<M>> IntoEntityObserver<M> for T {
    fn into_observer_for_entity(self, entity: Entity, world: &mut World) -> Observer {
        self.into_observer(world).with_entity(entity)
    }
}

// -----------------------------------------------------------------------------
// observer_system_runner

fn observer_system_runner<E: Event, B: Bundle, S: ObserverSystem<E, B>>(
    world: DeferredWorld,
    context: EventContext,
    observer: ObserverId,
    event_ptr: PtrMut,
    trigger_ptr: PtrMut,
) {
    let last_trigger = world.last_trigger();

    let world = world.unsafe_world();

    let state = unsafe { world.data_mut().observers.get_unchecked_mut(observer) };

    if state.last_trigger == last_trigger {
        return;
    }
    state.last_trigger = last_trigger;

    let trigger: &mut E::Trigger<'_> = unsafe { trigger_ptr.deref() };
    let event: &mut E = unsafe { event_ptr.deref() };

    let on: On<E, B> = On::new(event, trigger, observer, context);

    let system: *mut dyn ObserverSystem<E, B> = unsafe {
        let system: &mut dyn Any = state.system.as_mut();
        let system = system.downcast_mut::<S>().debug_checked_unwrap();
        &mut *system
    };

    unsafe {
        if let Err(err) = (*system).run_raw(on, world) {
            voker_utils::cold_path();
            let handler = state
                .error_handler
                .unwrap_or_else(|| world.read_only().fallback_error_handler().0);

            handler(
                err.into(),
                ErrorContext::Observer {
                    name: (*system).id().name(),
                    last_run: (*system).last_run(),
                },
            );
        }

        (*system).queue_deferred(world.deferred());
    }
}
