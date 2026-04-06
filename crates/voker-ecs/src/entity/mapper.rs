use std::vec::Vec;

use voker_utils::hash::{SparseHashMap, SparseHashSet};

use super::Entity;

// -----------------------------------------------------------------------------
// EntityHashMap

pub type EntityHashMap<T> = SparseHashMap<Entity, T>;

pub type EntityHashSet = SparseHashSet<Entity>;

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

impl EntityMapper for EntityHashMap<Entity> {
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

// -----------------------------------------------------------------------------
// EntitySet

pub trait EntitySet {
    fn map_entities(&mut self, mapper: &mut dyn EntityMapper);
}

impl EntitySet for Entity {
    fn map_entities(&mut self, mapper: &mut dyn EntityMapper) {
        *self = mapper.get_mapped(*self);
    }
}

impl<T> EntitySet for EntityHashMap<T> {
    fn map_entities(&mut self, mapper: &mut dyn EntityMapper) {
        let mut buffer = EntityHashMap::with_capacity(self.len());
        self.drain().for_each(|(e, v)| {
            buffer.insert(mapper.get_mapped(e), v);
        });
        *self = buffer;
    }
}

impl EntitySet for EntityHashSet {
    fn map_entities(&mut self, mapper: &mut dyn EntityMapper) {
        let mut buffer = EntityHashSet::with_capacity(self.len());
        self.iter().for_each(|e| {
            buffer.insert(mapper.get_mapped(*e));
        });
        *self = buffer;
    }
}

impl EntitySet for Vec<Entity> {
    fn map_entities(&mut self, mapper: &mut dyn EntityMapper) {
        self.iter_mut().for_each(|e| {
            *e = mapper.get_mapped(*e);
        });
    }
}
