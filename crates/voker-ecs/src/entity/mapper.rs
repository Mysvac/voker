use voker_utils::hash::SparseHashMap;

use super::Entity;

// -----------------------------------------------------------------------------
// EntityHashMap

pub type EntityMap<T> = SparseHashMap<Entity, T>;

// -----------------------------------------------------------------------------
// EntityMapper

/// An implementor of this trait knows how to map an [`Entity`] into another [`Entity`].
///
/// Usually this is done by using an [`EntityMap<Entity>`] to map source entities
/// (mapper inputs) to the current world's entities (mapper outputs).
pub trait EntityMapper {
    /// Returns the "target" entity that maps to the given `source`.
    fn get_mapped(&mut self, source: Entity) -> Entity;

    /// Maps the `target` entity to the given `source`.
    ///
    /// For some implementations this might not actually determine the result
    /// of [`EntityMapper::get_mapped`].
    fn set_mapped(&mut self, source: Entity, target: Entity);
}

impl EntityMapper for () {
    #[inline]
    fn get_mapped(&mut self, source: Entity) -> Entity {
        source
    }

    #[inline]
    fn set_mapped(&mut self, _source: Entity, _target: Entity) {}
}

impl EntityMapper for (Entity, Entity) {
    #[inline]
    fn get_mapped(&mut self, source: Entity) -> Entity {
        if source == self.0 { self.1 } else { source }
    }

    #[inline]
    fn set_mapped(&mut self, _source: Entity, _target: Entity) {}
}

impl EntityMapper for EntityMap<Entity> {
    fn get_mapped(&mut self, source: Entity) -> Entity {
        self.get(&source).cloned().unwrap_or(source)
    }

    fn set_mapped(&mut self, source: Entity, target: Entity) {
        self.insert(source, target);
    }
}

impl EntityMapper for &mut dyn EntityMapper {
    fn get_mapped(&mut self, source: Entity) -> Entity {
        (*self).get_mapped(source)
    }

    fn set_mapped(&mut self, source: Entity, target: Entity) {
        (*self).set_mapped(source, target);
    }
}
