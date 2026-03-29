use voker_utils::hash::NoOpHashMap;

use super::SystemId;
use crate::entity::Entity;

#[derive(Default)]
pub struct SystemRegistry {
    mapper: NoOpHashMap<SystemId, Entity>,
}

impl SystemRegistry {
    pub const fn new() -> Self {
        Self {
            mapper: NoOpHashMap::new(),
        }
    }
    /// Inserts or replaces the entity binding of a system name.
    pub fn insert(&mut self, name: SystemId, entity: Entity) {
        self.mapper.insert(name, entity);
    }

    /// Removes a system binding and returns its entity if it exists.
    pub fn remove(&mut self, name: SystemId) -> Option<Entity> {
        self.mapper.remove(&name)
    }

    /// Returns the entity currently bound to a system name.
    pub fn get(&self, name: SystemId) -> Option<Entity> {
        self.mapper.get(&name).copied()
    }
}
