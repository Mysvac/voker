use super::ComponentId;
use crate::entity::Entity;
use crate::utils::DebugLocation;
use crate::world::DeferredWorld;

pub type ComponentHook = fn(DeferredWorld, ComponentHookContext);

#[derive(Debug, Clone, Copy)]
pub struct ComponentHookContext {
    pub id: ComponentId,
    pub entity: Entity,
    pub caller: DebugLocation,
}

#[derive(Debug, Clone)]
pub struct ComponentHooks {
    pub on_add: Option<ComponentHook>,
    pub on_clone: Option<ComponentHook>,
    pub on_insert: Option<ComponentHook>,
    pub on_remove: Option<ComponentHook>,
    pub on_discard: Option<ComponentHook>,
    pub on_despawn: Option<ComponentHook>,
}

// spawn: add + insert
// insert: discard(option) + insert
// remove: discard + remove
// despawn: discard + remove + despawn
// clone: clone + add + insert
