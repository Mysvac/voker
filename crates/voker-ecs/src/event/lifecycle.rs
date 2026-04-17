use voker_ecs_derive::EntityEvent;

use super::EntityComponentsTrigger;
use crate::entity::Entity;

/// Lifecycle event emitted when a component is inserted onto an entity that
/// does not already contain that component.
///
/// This runs before [`Insert`].
///
/// See [`ComponentHooks::on_add`](crate::component::ComponentHooks::on_add)
/// for hook-level behavior.
#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Add {
    /// The target entity receiving the newly added component.
    #[event_target]
    pub entity: Entity,
}

/// Lifecycle event emitted when an entity is cloned and a component value is
/// copied into the clone.
///
/// This runs before [`Add`] and [`Insert`] for cloned components.
///
/// See [`ComponentHooks::on_clone`](crate::component::ComponentHooks::on_clone)
/// for hook-level behavior.
#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Clone {
    /// The cloned target entity receiving the copied component.
    #[event_target]
    pub entity: Entity,
}

/// Lifecycle event emitted when a component value is inserted into an entity,
/// regardless of whether that component already existed.
///
/// If the component was newly added, [`Add`] runs before this event.
///
/// See [`ComponentHooks::on_insert`](crate::component::ComponentHooks::on_insert)
/// for hook-level behavior.
#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Insert {
    /// The target entity receiving the inserted component value.
    #[event_target]
    pub entity: Entity,
}

/// Lifecycle event emitted when a component is about to be replaced or removed
/// from an entity.
///
/// This runs before the old value is discarded, so observers can still inspect
/// pre-change state.
///
/// See [`ComponentHooks::on_discard`](crate::component::ComponentHooks::on_discard)
/// for hook-level behavior.
#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Discard {
    /// The entity that currently holds the component being discarded.
    #[event_target]
    pub entity: Entity,
}

/// Lifecycle event emitted when a component is removed from an entity.
///
/// This runs before removal is finalized, so observers can still inspect
/// relevant component state in the same flush.
///
/// See [`ComponentHooks::on_remove`](crate::component::ComponentHooks::on_remove)
/// for hook-level behavior.
#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Remove {
    /// The entity losing the removed component.
    #[event_target]
    pub entity: Entity,
}

/// Lifecycle event emitted for each component on an entity when that entity is
/// despawned.
///
/// See [`ComponentHooks::on_despawn`](crate::component::ComponentHooks::on_despawn)
/// for hook-level behavior.
#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Despawn {
    /// The entity being despawned.
    #[event_target]
    pub entity: Entity,
}
