use alloc::collections::{BTreeMap, BTreeSet, LinkedList, VecDeque};
use core::hash::{BuildHasher, Hash};

use alloc::vec::Vec;
use voker_utils::extra::{ArrayDeque, BlockList};
use voker_utils::hash::{HashMap, HashSet, NoopHashMap, NoopHashSet, SparseHashMap, SparseHashSet};
use voker_utils::index::{IndexMap, IndexSet, SparseIndexMap, SparseIndexSet};
use voker_utils::vec::{ArrayVec, FastVec, FastVecData, SmallVec};

use crate::world::World;

use super::{Entity, EntityHashMap};

// -----------------------------------------------------------------------------
// MapEntities

/// Operation to map all contained [`Entity`] fields in a type to new values.
///
/// Components use [`Component::map_entities`] to map entities in the context
/// of scenes and entity cloning, which generally uses [`MapEntities`] internally
/// to map each field.
///
/// [`Component::map_entities`]: crate::component::Component::map_entities
pub trait MapEntities {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E);
}

// -----------------------------------------------------------------------------
// EntityMapper

/// An implementor of this trait knows how to map an [`Entity`] into another [`Entity`].
///
/// Usually this is done by using an [`EntityHashMap<Entity>`] to map source entities
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

// -----------------------------------------------------------------------------
// SceneEntityMapper

/// A wrapper for [`EntityHashMap<Entity>`], augmenting it with the ability to allocate new
/// [`Entity`] references in a destination world.
///
/// These newly allocated references are guaranteed to never point to any living entity in that world.
///
/// References are allocated by returning increasing generations starting from an internally
/// initialized base [`Entity`]. After it is finished being used, this entity is despawned and
/// the requisite number of generations reserved.
pub struct SceneEntityMapper<'m> {
    /// A mapping from one set of entities to another.
    ///
    /// This is typically used to coordinate data transfer between sets of entities,
    /// such as between a scene and the world or over the network. This is required as
    /// [`Entity`] identifiers are opaque; you cannot and do not want to reuse identifiers
    /// directly.
    ///
    /// On its own, a [`EntityHashMap<Entity>`] is not capable of allocating new entity identifiers,
    /// which is needed to map references to entities that lie outside the source entity set. This
    /// functionality can be accessed through [`SceneEntityMapper::world_scope()`].
    mapper: &'m mut EntityHashMap<Entity>,
    /// A base [`Entity`] used to allocate new references.
    template: Entity,
    /// The number of generations this mapper has allocated thus far.
    allocator: u32,
}

impl<'m> SceneEntityMapper<'m> {
    /// Gets a reference to the underlying [`EntityHashMap<Entity>`].
    pub fn inner(&'m self) -> &'m EntityHashMap<Entity> {
        self.mapper
    }

    /// Gets a mutable reference to the underlying [`EntityHashMap<Entity>`].
    pub fn inner_mut(&'m mut self) -> &'m mut EntityHashMap<Entity> {
        self.mapper
    }

    /// Creates a new [`SceneEntityMapper`] by reserving a temporary base [`Entity`]
    /// in the provided [`World`].
    pub fn new(world: &World, mapper: &'m mut EntityHashMap<Entity>) -> Self {
        Self {
            mapper,
            template: world.allocator.alloc(),
            allocator: 0,
        }
    }

    /// Reserves the allocated references to dead entities within the world. This frees the temporary base
    /// [`Entity`] while reserving extra generations. Because this makes the [`SceneEntityMapper`] unable to
    /// safely allocate any more references, this method takes ownership of `self` in order to render it unusable.
    pub fn finish(self, world: &mut World) {
        // SAFETY: We never constructed the entity and never released it for something else to construct.
        let reuse_row = unsafe { world.entities.free(self.template.id(), self.allocator) };
        world.allocator.free(reuse_row);
    }

    /// Creates a [`SceneEntityMapper`] from a provided [`World`] and
    /// [`EntityHashMap<Entity>`], then calls the provided function with it.
    ///
    /// This allows one to allocate new entity references in this [`World`] that are guaranteed
    /// to never point at a living entity now or in the future. This functionality is useful for
    /// safely mapping entity identifiers that point at entities outside the source world. The
    /// passed function, `f`, is called within the scope of this world. Its return value is then
    /// returned from `world_scope` as the generic type parameter `R`.
    pub fn world_scope<R>(
        world: &mut World,
        mapper: &'m mut EntityHashMap<Entity>,
        f: impl FnOnce(&mut World, &mut Self) -> R,
    ) -> R {
        let mut mapper = Self::new(world, mapper);
        let result = f(world, &mut mapper);
        mapper.finish(world);
        result
    }
}

// -----------------------------------------------------------------------------
// MapEntities Implementation

impl MapEntities for () {
    fn map_entities<E: EntityMapper>(&mut self, _: &mut E) {}
}

impl MapEntities for Entity {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        *self = entity_mapper.get_mapped(*self);
    }
}

impl<T: MapEntities> MapEntities for Option<T> {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        if let Some(entities) = self {
            entities.map_entities(entity_mapper);
        }
    }
}

macro_rules! impl_map_entities_for_map {
    ($($ty:tt)*) => {
        impl $($ty)* {
            fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
                *self = core::mem::take(self)
                    .into_iter()
                    .map(|(mut key_entities, mut value_entities)| {
                        key_entities.map_entities(entity_mapper);
                        value_entities.map_entities(entity_mapper);
                        (key_entities, value_entities)
                    })
                    .collect();
            }
        }
    };
}

impl_map_entities_for_map! {
    <K: MapEntities + Eq + Hash, V: MapEntities, S: BuildHasher + Default> MapEntities for HashMap<K, V, S>
}

impl_map_entities_for_map! {
    <K: MapEntities + Eq + Hash, V: MapEntities> MapEntities for SparseHashMap<K, V>
}

impl_map_entities_for_map! {
    <K: MapEntities + Eq + Hash, V: MapEntities> MapEntities for NoopHashMap<K, V>
}

impl_map_entities_for_map! {
    <K: MapEntities + Eq + Hash, V: MapEntities, S: BuildHasher + Default> MapEntities for IndexMap<K, V, S>
}

impl_map_entities_for_map! {
    <K: MapEntities + Eq + Hash, V: MapEntities> MapEntities for SparseIndexMap<K, V>
}

impl_map_entities_for_map! {
    <K: MapEntities + Ord, V: MapEntities> MapEntities for BTreeMap<K, V>
}

macro_rules! impl_map_entities_for_set {
    ($($ty:tt)*) => {
        impl $($ty)* {
            fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
                *self = core::mem::take(self)
                    .into_iter()
                    .map(|mut entities| {
                        entities.map_entities(entity_mapper);
                        entities
                    })
                    .collect();
            }
        }
    };
}

impl_map_entities_for_set! {
    <T: MapEntities + Eq + Hash, S: BuildHasher + Default> MapEntities for HashSet<T, S>
}

impl_map_entities_for_set! {
    <T: MapEntities + Eq + Hash> MapEntities for SparseHashSet<T>
}

impl_map_entities_for_set! {
    <T: MapEntities + Eq + Hash> MapEntities for NoopHashSet<T>
}

impl_map_entities_for_set! {
    <T: MapEntities + Eq + Hash, S: BuildHasher + Default> MapEntities for IndexSet<T, S>
}

impl_map_entities_for_set! {
    <T: MapEntities + Eq + Hash> MapEntities for SparseIndexSet<T>
}

impl_map_entities_for_set! {
    <T: MapEntities + Ord> MapEntities for BTreeSet<T>
}

macro_rules! impl_map_entities_for_list {
    ($($ty:tt)*) => {
        impl $($ty)* {
            fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
                for entities in self.iter_mut() {
                    entities.map_entities(entity_mapper);
                }
            }
        }
    };
}

impl_map_entities_for_list!(<T: MapEntities, const N: usize> MapEntities for [T; N]);
impl_map_entities_for_list!(<T: MapEntities, const N: usize> MapEntities for ArrayVec<T, N>);
impl_map_entities_for_list!(<T: MapEntities, const N: usize> MapEntities for SmallVec<T, N>);
impl_map_entities_for_list!(<T: MapEntities, const N: usize> MapEntities for FastVec<T, N>);
impl_map_entities_for_list!(<T: MapEntities, const N: usize> MapEntities for FastVecData<T, N>);
impl_map_entities_for_list!(<T: MapEntities, const N: usize> MapEntities for ArrayDeque<T, N>);
impl_map_entities_for_list!(<T: MapEntities> MapEntities for Vec<T>);
impl_map_entities_for_list!(<T: MapEntities> MapEntities for VecDeque<T>);
impl_map_entities_for_list!(<T: MapEntities> MapEntities for BlockList<T>);
impl_map_entities_for_list!(<T: MapEntities> MapEntities for LinkedList<T>);
impl_map_entities_for_list!(<T: MapEntities> MapEntities for &mut [T]);

// -----------------------------------------------------------------------------
// EntityMapper Implementation

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

impl EntityMapper for SceneEntityMapper<'_> {
    /// Returns the corresponding mapped entity or reserves a new dead entity ID in the current world if it is absent.
    fn get_mapped(&mut self, source: Entity) -> Entity {
        if let Some(&mapped) = self.mapper.get(&source) {
            return mapped;
        }

        let id = self.template.id();
        let tag = self.template.tag().wrapping_add(self.allocator);

        // this new entity reference is specifically designed to never represent any living entity
        let new = Entity::new(id, tag);
        // Starting from 0, should be no overflow.
        self.allocator += 1;

        self.mapper.insert(source, new);

        new
    }

    fn set_mapped(&mut self, source: Entity, target: Entity) {
        self.mapper.insert(source, target);
    }
}
