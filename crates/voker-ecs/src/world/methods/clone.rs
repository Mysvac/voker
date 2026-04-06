use crate::entity::{Entity, EntityError};
use crate::world::World;

impl World {
    pub fn spawn_clone(&mut self, _entity: Entity) -> Result<Entity, EntityError> {
        todo!()
    }
}
