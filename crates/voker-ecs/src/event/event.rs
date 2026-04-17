#![expect(clippy::module_inception, reason = "For better structure.")]

use super::{EventId, Trigger};
use crate::entity::Entity;
use crate::utils::DebugLocation;

/// Runtime context attached to a triggered event.
///
/// This metadata is passed through observer execution and is primarily useful
/// for diagnostics, tracing, and error reporting.
#[derive(Debug, Clone, Copy)]
pub struct EventContext {
    /// Unique runtime identifier of the event type being triggered.
    pub id: EventId,
    /// Call-site information captured when the event was triggered.
    pub caller: DebugLocation,
}

/// A triggerable event type.
///
/// `Event` is the core trait used by the observer system. Triggering an event
/// runs observers selected by the event's associated [`Trigger`] type.
///
/// In most cases, you should derive this trait:
///
/// ```no_run
/// # use voker_ecs::prelude::*;
/// #[derive(Event)]
/// struct AppStarted;
/// ```
///
/// # Trigger Behavior
///
/// Every event defines an associated trigger via [`Event::Trigger`]. The
/// trigger controls:
/// - which observers run,
/// - what trigger state is provided to observers,
/// - and the dispatch order.
///
/// If you are not implementing ECS internals, you usually do not need to
/// implement [`Trigger`] manually.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `Event`",
    label = "invalid `Event`",
    note = "consider annotating `{Self}` with `#[derive(Event)]`"
)]
pub trait Event: Send + Sync + Sized + 'static {
    /// Dispatch strategy and trigger payload for this event type.
    ///
    /// This type defines how observers are discovered and invoked.
    type Trigger<'a>: Trigger<Self>;
}

/// An [`Event`] that targets a specific [`Entity`].
///
/// Entity events run both normal global observers and entity-scoped observers
/// that watch the target entity.
///
/// In most cases, you should derive this trait:
///
/// ```no_run
/// # use voker_ecs::prelude::*;
/// #[derive(EntityEvent)]
/// struct Hit {
///     #[event_target]
///     entity: Entity,
/// }
/// ```
///
/// The derive macro can infer the target from common forms (for example a
/// field named `entity`). See derive documentation for full target-selection
/// rules.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `EntityEvent`",
    label = "invalid `EntityEvent`",
    note = "consider annotating `{Self}` with `#[derive(EntityEvent)]`"
)]
pub trait EntityEvent: Event {
    /// Returns the target [`Entity`] of this event.
    ///
    /// Trigger implementations use this value to select entity-scoped
    /// observers.
    fn event_target(&self) -> Entity;
}

/// Mutable-target variant of [`EntityEvent`].
///
/// Most entity events are immutable while being dispatched. Implementing this
/// trait allows the target entity to be updated, which is required by
/// propagation-oriented trigger implementations.
///
/// You typically do not implement this trait manually. Use derive attributes
/// that enable propagation support instead.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not an `EntityEventMut`",
    label = "invalid `EntityEventMut`",
    note = "consider annotating `{Self}` with `#[entity_event(propagate)]`"
)]
pub trait EntityEventMut: EntityEvent {
    /// Sets the target [`Entity`] of this event.
    ///
    /// Note: Changing the target during observer execution does not
    /// automatically retarget already-resolved observer sets.
    fn set_event_target(&mut self, entity: Entity);
}
