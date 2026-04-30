use alloc::vec::Vec;

use slotmap::SlotMap;
use voker_utils::hash::SparseHashMap;

use super::ObserverRunner;
use crate::component::ComponentId;
use crate::entity::EntityHashMap;
use crate::event::EventId;
use crate::observer::{Observer, ObserverId};

// -----------------------------------------------------------------------------
// ObserverRunner & ObserverMap

/// Map from observer id to runner function.
pub type ObserverMap = SparseHashMap<ObserverId, ObserverRunner>;

// -----------------------------------------------------------------------------
// CachedComponentObservers

/// Cached observers grouped by one component target.
#[derive(Default, Debug)]
pub struct CachedComponentObservers {
    // Observers watching for events targeting this component, but not a specific entity
    pub(super) global_observers: ObserverMap,
    // Observers watching for events targeting this component on a specific entity
    pub(super) entity_component_observers: EntityHashMap<ObserverMap>,
}

impl CachedComponentObservers {
    /// Returns observers watching for events targeting this component, but not a specific entity
    pub fn global_observers(&self) -> &ObserverMap {
        &self.global_observers
    }

    /// Returns observers watching for events targeting this component on a specific entity
    pub fn entity_component_observers(&self) -> &EntityHashMap<ObserverMap> {
        &self.entity_component_observers
    }
}

// -----------------------------------------------------------------------------
// CachedObservers

/// Cached observers grouped for one event type.
///
/// This structure powers fast trigger dispatch by splitting observers into:
/// - global observers,
/// - component-scoped observers,
/// - entity-scoped observers.
#[derive(Default, Debug)]
pub struct CachedObservers {
    /// Observers watching for any time this event is triggered, regardless of target.
    /// These will also respond to events targeting specific components or entities
    pub(super) global_observers: ObserverMap,
    /// Observers watching for triggers of events for a specific component
    pub(super) component_observers: SparseHashMap<ComponentId, CachedComponentObservers>,
    /// Observers watching for triggers of events for a specific entity
    pub(super) entity_observers: EntityHashMap<ObserverMap>,
}

impl CachedObservers {
    /// Creates an empty cache bucket for one event type.
    const fn new() -> Self {
        Self {
            global_observers: ObserverMap::new(),
            component_observers: SparseHashMap::new(),
            entity_observers: EntityHashMap::new(),
        }
    }

    /// Observers watching for any time this event is triggered, regardless of target.
    /// These will also respond to events targeting specific components or entities
    pub fn global_observers(&self) -> &ObserverMap {
        &self.global_observers
    }

    /// Returns observers watching for triggers of events for a specific component.
    pub fn component_observers(&self) -> &SparseHashMap<ComponentId, CachedComponentObservers> {
        &self.component_observers
    }

    /// Returns observers watching for triggers of events for a specific entity.
    pub fn entity_observers(&self) -> &EntityHashMap<ObserverMap> {
        &self.entity_observers
    }
}

// -----------------------------------------------------------------------------
// Observers

/// Observer registry and per-event dispatch caches.
#[derive(Debug)]
pub struct Observers {
    pub(crate) runners: Vec<CachedObservers>,
    pub(crate) observers: SlotMap<ObserverId, Observer>,
}

impl Observers {
    pub(crate) fn new() -> Self {
        Self {
            runners: Vec::new(),
            observers: SlotMap::default(),
        }
    }

    pub(crate) unsafe fn get_unchecked_mut(&mut self, id: ObserverId) -> &mut Observer {
        unsafe { self.observers.get_unchecked_mut(id) }
    }

    pub(crate) fn register(&mut self, observer: Observer) -> ObserverId {
        let id = self.observers.insert(observer);

        let observer = unsafe { self.observers.get_unchecked_mut(id) };
        let runner = observer.runner;
        let event_id = observer.event_id;

        if event_id.index() >= self.runners.len() {
            core::hint::cold_path();
            self.runners.resize_with(event_id.index() + 1, CachedObservers::new);
        }

        let observers = unsafe { self.runners.get_unchecked_mut(event_id.index()) };

        if observer.entities.is_empty() {
            if observer.components.is_empty() {
                observers.global_observers.insert(id, runner);
            } else {
                for &cid in observer.components.iter() {
                    observers
                        .component_observers
                        .entry(cid)
                        .or_default()
                        .global_observers
                        .insert(id, runner);
                }
            }
        } else {
            if observer.components.is_empty() {
                for &e in observer.entities.iter() {
                    observers.entity_observers.entry(e).or_default().insert(id, runner);
                }
            } else {
                for &cid in observer.components.iter() {
                    for &e in observer.entities.iter() {
                        observers
                            .component_observers
                            .entry(cid)
                            .or_default()
                            .entity_component_observers
                            .entry(e)
                            .or_default()
                            .insert(id, runner);
                    }
                }
            }
        }

        id
    }
}

impl Observers {
    /// Returns cached observers for the given event id, if registered.
    pub fn get_observers(&self, event_id: EventId) -> Option<&CachedObservers> {
        self.runners.get(event_id.index())
    }

    /// Returns an observer by id.
    pub fn get(&self, id: ObserverId) -> Option<&Observer> {
        self.observers.get(id)
    }

    /// Returns an observer by id without bounds checks.
    ///
    /// # Safety
    /// `id` must refer to a live observer in this storage.
    pub unsafe fn get_unchecked(&self, id: ObserverId) -> &Observer {
        unsafe { self.observers.get_unchecked(id) }
    }
}
