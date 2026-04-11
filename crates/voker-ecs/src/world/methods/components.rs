use crate::entity::Entity;
use crate::world::{GetComponents, World};

impl World {
    pub fn get<C: GetComponents>(&self, entity: Entity) -> Option<C::Raw<'_>> {
        self.get_entity_ref(entity).ok()?.get::<C>()
    }

    pub fn get_ref<C: GetComponents>(&self, entity: Entity) -> Option<C::Ref<'_>> {
        self.get_entity_ref(entity).ok()?.into_ref::<C>()
    }

    pub fn get_mut<C: GetComponents>(&mut self, entity: Entity) -> Option<C::Mut<'_>> {
        self.get_entity_mut(entity).ok()?.into_mut::<C>()
    }
}
