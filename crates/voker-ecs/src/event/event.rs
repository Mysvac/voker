#![expect(clippy::module_inception, reason = "For better structure.")]

use super::{EventId, Trigger};
use crate::entity::Entity;
use crate::utils::DebugLocation;

#[derive(Debug, Clone, Copy)]
pub struct EventContext {
    pub id: EventId,
    pub caller: DebugLocation,
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `Event`",
    label = "invalid `Event`",
    note = "consider annotating `{Self}` with `#[derive(Event)]`"
)]
pub trait Event: Send + Sync + Sized + 'static {
    type Trigger<'a>: Trigger<Self>;
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `EntityEvent`",
    label = "invalid `EntityEvent`",
    note = "consider annotating `{Self}` with `#[derive(EntityEvent)]`"
)]
pub trait EntityEvent: Event {
    fn event_target(&self) -> Entity;
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `EntityEventMut`",
    label = "invalid `EntityEventMut`",
    note = "consider annotating `{Self}` with `#[entity_event(propagate)]`"
)]
pub trait EntityEventMut: EntityEvent {
    fn set_event_target(&mut self, entity: Entity);
}
