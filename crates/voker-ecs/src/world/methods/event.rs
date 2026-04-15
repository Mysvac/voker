use core::any::TypeId;

use crate::event::{Event, EventId};
use crate::world::World;

impl World {
    /// Registers a event type in the global message registry.
    ///
    /// Adding an observer will automatically call this function.
    pub fn register_event<E: Event>(&mut self) -> EventId {
        self.events.register::<E>()
    }

    /// Looks up a event ID by its TypeId.
    pub fn get_event_id<E: Event>(&self) -> Option<EventId> {
        self.events.get_id(TypeId::of::<E>())
    }
}
