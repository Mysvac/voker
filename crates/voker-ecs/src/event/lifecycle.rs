use voker_ecs_derive::EntityEvent;

use super::EntityComponentsTrigger;
use crate::entity::Entity;

#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Add {
    #[event_target]
    pub entity: Entity,
}

#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Clone {
    #[event_target]
    pub entity: Entity,
}

#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Insert {
    #[event_target]
    pub entity: Entity,
}

#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Discard {
    #[event_target]
    pub entity: Entity,
}

#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Remove {
    #[event_target]
    pub entity: Entity,
}

#[derive(EntityEvent, Debug, Clone)]
#[entity_event(trigger = EntityComponentsTrigger<'a>)]
pub struct Despawn {
    #[event_target]
    pub entity: Entity,
}
