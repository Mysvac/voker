use crate::entity::Entity;
use crate::world::{GetComponents, World};

impl World {
    /// Returns raw shared component access for `entity` when present.
    ///
    /// This is a convenience wrapper over [`World::get_entity_ref`] and
    /// [`GetComponents::get`]. Returns `None` when the entity is not spawned or
    /// when the requested component pattern is not available.
    pub fn get<C: GetComponents>(&self, entity: Entity) -> Option<C::Raw<'_>> {
        self.get_entity_ref(entity).ok()?.get::<C>()
    }

    /// Returns change-aware shared component access for `entity` when present.
    ///
    /// This variant carries change-detection context (`last_run`/`this_run`).
    /// Returns `None` when the entity is not spawned or the component pattern
    /// does not match.
    pub fn get_ref<C: GetComponents>(&self, entity: Entity) -> Option<C::Ref<'_>> {
        self.get_entity_ref(entity).ok()?.into_ref::<C>()
    }

    /// Returns change-aware mutable component access for `entity` when present.
    ///
    /// Returns `None` when the entity is not spawned or the component pattern
    /// does not match.
    pub fn get_mut<C: GetComponents>(&mut self, entity: Entity) -> Option<C::Mut<'_>> {
        self.get_entity_mut(entity).ok()?.into_mut::<C>()
    }
}
