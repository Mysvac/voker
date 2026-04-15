use alloc::collections::{BTreeSet, VecDeque};
use core::iter::Copied;

use alloc::vec::Vec;
use voker_utils::vec::SmallVec;

use crate::entity::{Entity, EntityHashSet, EntityIndexSet};

// -----------------------------------------------------------------------------
// RelationshipSourceSet & OrderedRelationshipSourceSet

/// Abstract source-entity collection used by [`RelationshipTarget`] caches.
///
/// Implementations define storage semantics for reverse links:
/// - single-source sets (`Entity`, `Option<Entity`) for one-to-one links
/// - multi-source sets (`Vec<Entity>`, sets, queues) for one-to-many links
///
/// [`RelationshipTarget`]: crate::relationship::RelationshipTarget
pub trait RelationshipSourceSet {
    type SourceIter<'a>: Iterator<Item = Entity>
    where
        Self: 'a;

    const SINGLE_ENTITY: bool;

    /// Creates a new empty instance.
    ///
    /// If possible, reserve specific capacity.
    fn with_hint(size_hint: usize) -> Self;

    /// Inserts a entity if not exists.
    ///
    /// If the collection is ordered, this will never reorder other entities.
    ///
    /// - Return `true` if the entity did not exist previously.
    /// - Return `false` if this entity already exists.
    fn insert(&mut self, entity: Entity) -> bool;

    /// Removes a entity if exists.
    ///
    /// If the collection is ordered, this will never reorder other entities.
    ///
    /// - Return `true` if this entity exists.
    /// - Return `false` if this entity does not exist.
    fn remove(&mut self, entity: Entity) -> bool;

    /// Clears the collection.
    fn clear(&mut self);

    /// Iterates all entities in the collection.
    fn iter(&self) -> Self::SourceIter<'_>;

    /// Returns the current length of the collection.
    fn len(&self) -> usize;

    /// Returns true if the collection contains no entities.
    fn is_empty(&self) -> bool;

    /// Reserves capacity for at least `additional` more entities to be inserted.
    ///
    /// Not all collections support this operation, in which case it is a no-op.
    fn reserve(&mut self, additional: usize);

    /// Reserves capacity for at least `additional` more entities to be inserted.
    ///
    /// Not all collections support this operation, in which case it is a no-op.
    fn shrink_to_fit(&mut self);

    /// For one-to-one links, the old entity should be removed before insert new one.
    ///
    /// Return `None` for one-to-many links or when no entity needs to be removed.
    #[inline]
    fn remove_before_insert(&self) -> Option<Entity> {
        None
    }
}

/// Extension trait for ordered source collections.
///
/// These APIs expose positional editing operations for queue/list-like
/// relationship targets.
pub trait OrderedRelationshipSourceSet: RelationshipSourceSet {
    /// Inserts the entity at a specific index.
    ///
    /// This will never reorder other entities.
    ///
    /// If the index is too large, the entity will be added to the end of the collection.
    fn insert_at(&mut self, index: usize, entity: Entity);

    /// Removes the entity at the specified index if it exists.
    ///
    /// This will never reorder other entities.
    ///
    /// If the index is too large, this function is no-op (return `None`).
    fn remove_at(&mut self, index: usize) -> Option<Entity>;

    /// Inserts the entity at a specific index.
    ///
    /// This is faster but may reorder other entities.
    ///
    /// If the index is too large, the entity will be added to the end of the collection.
    fn insert_at_unstable(&mut self, index: usize, entity: Entity);

    /// Removes the entity at the specified index if it exists.
    ///
    /// This is faster but may reorder other entities.
    ///
    /// If the index is too large, this function is no-op (return `None`).
    fn remove_at_unstable(&mut self, index: usize) -> Option<Entity>;

    /// This places the most recently added entity at the particular index.
    ///
    /// This will do nothing if there are no entities in the collection.
    fn place_most_recent(&mut self, index: usize);

    /// This places the given entity at the particular index.
    ///
    /// This will do nothing if the entity is not in the collection.
    ///
    /// If the index is out of bounds, this will put the entity at the end.
    fn place(&mut self, entity: Entity, index: usize);

    /// Adds the entity at index 0.
    fn push_front(&mut self, entity: Entity);

    /// Adds the entity to the back of the collection.
    fn push_back(&mut self, entity: Entity);

    /// Removes the first entity.
    fn pop_front(&mut self) -> Option<Entity>;

    /// Removes the last entity.
    fn pop_back(&mut self) -> Option<Entity>;
}

// -----------------------------------------------------------------------------
// Entity / Option<Entity>

impl RelationshipSourceSet for Entity {
    type SourceIter<'a> = core::option::IntoIter<Entity>;

    const SINGLE_ENTITY: bool = true;

    #[inline]
    fn with_hint(_: usize) -> Self {
        Entity::PLACEHOLDER
    }

    #[inline]
    fn insert(&mut self, entity: Entity) -> bool {
        if *self == entity {
            return false;
        }
        *self = entity;
        true
    }

    #[inline]
    fn remove(&mut self, entity: Entity) -> bool {
        if *self == entity {
            *self = Entity::PLACEHOLDER;
            return true;
        }
        false
    }

    #[inline]
    fn clear(&mut self) {
        *self = Entity::PLACEHOLDER;
    }

    #[inline]
    fn iter(&self) -> Self::SourceIter<'_> {
        (!self.is_empty()).then_some(*self).into_iter()
    }

    #[inline]
    fn len(&self) -> usize {
        (*self != Entity::PLACEHOLDER) as usize
    }

    #[inline]
    fn is_empty(&self) -> bool {
        *self == Entity::PLACEHOLDER
    }

    #[inline]
    fn reserve(&mut self, _: usize) {}

    #[inline]
    fn shrink_to_fit(&mut self) {}

    #[inline]
    fn remove_before_insert(&self) -> Option<Entity> {
        if *self == Entity::PLACEHOLDER {
            None
        } else {
            Some(*self)
        }
    }
}

impl RelationshipSourceSet for Option<Entity> {
    type SourceIter<'a> = core::option::IntoIter<Entity>;

    const SINGLE_ENTITY: bool = true;

    #[inline]
    fn with_hint(_: usize) -> Self {
        None
    }

    #[inline]
    fn insert(&mut self, entity: Entity) -> bool {
        if *self == Some(entity) {
            return false;
        }
        *self = Some(entity);
        true
    }

    #[inline]
    fn remove(&mut self, entity: Entity) -> bool {
        if *self == Some(entity) {
            *self = None;
            return true;
        }
        false
    }

    #[inline]
    fn clear(&mut self) {
        *self = None;
    }

    #[inline]
    fn iter(&self) -> Self::SourceIter<'_> {
        (*self).into_iter()
    }

    #[inline]
    fn len(&self) -> usize {
        self.is_some() as usize
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.is_none()
    }

    #[inline]
    fn reserve(&mut self, _: usize) {}

    #[inline]
    fn shrink_to_fit(&mut self) {}

    #[inline]
    fn remove_before_insert(&self) -> Option<Entity> {
        *self
    }
}

// -----------------------------------------------------------------------------
// Ordered Container

impl RelationshipSourceSet for Vec<Entity> {
    type SourceIter<'a> = Copied<core::slice::Iter<'a, Entity>>;

    const SINGLE_ENTITY: bool = false;

    fn with_hint(size_hint: usize) -> Self {
        Vec::with_capacity(size_hint)
    }

    fn insert(&mut self, entity: Entity) -> bool {
        use crate::utils::contains_entity;
        if contains_entity(entity, self.as_slice()) {
            false
        } else {
            Vec::push(self, entity);
            true
        }
    }

    fn remove(&mut self, entity: Entity) -> bool {
        use crate::utils::position_entity;
        match position_entity(entity, self.as_slice()) {
            Some(index) => {
                self.remove(index);
                true
            }
            None => false,
        }
    }

    fn clear(&mut self) {
        Vec::clear(self);
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        self.as_slice().iter().copied()
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn is_empty(&self) -> bool {
        Vec::is_empty(self)
    }

    fn reserve(&mut self, additional: usize) {
        Vec::reserve(self, additional);
    }

    fn shrink_to_fit(&mut self) {
        Vec::shrink_to_fit(self);
    }
}

impl OrderedRelationshipSourceSet for Vec<Entity> {
    fn insert_at(&mut self, index: usize, entity: Entity) {
        let index = index.min(Vec::len(self));
        Vec::insert(self, index, entity);
    }

    fn remove_at(&mut self, index: usize) -> Option<Entity> {
        let in_bound = index < Vec::len(self);
        in_bound.then(|| Vec::remove(self, index))
    }

    fn insert_at_unstable(&mut self, index: usize, entity: Entity) {
        let len = Vec::len(self);
        Vec::push(self, entity);
        if index < len {
            self.swap(index, len);
        }
    }

    fn remove_at_unstable(&mut self, index: usize) -> Option<Entity> {
        if index < Vec::len(self) {
            Some(self.swap_remove(index))
        } else {
            None
        }
    }

    fn place_most_recent(&mut self, index: usize) {
        if let Some(entity) = self.pop() {
            let index = index.min(Vec::len(self));
            Vec::insert(self, index, entity);
        }
    }

    fn place(&mut self, entity: Entity, index: usize) {
        let mut slice = self.as_slice().iter();
        if let Some(current) = slice.position(|e| *e == entity) {
            Vec::remove(self, current);
            let index = index.min(Vec::len(self));
            Vec::insert(self, index, entity);
        };
    }

    fn push_front(&mut self, entity: Entity) {
        Vec::insert(self, 0, entity);
    }

    fn push_back(&mut self, entity: Entity) {
        Vec::push(self, entity);
    }

    fn pop_front(&mut self) -> Option<Entity> {
        let contains = Vec::is_empty(self);
        contains.then(|| Vec::remove(self, 0))
    }

    fn pop_back(&mut self) -> Option<Entity> {
        Vec::pop(self)
    }
}

impl RelationshipSourceSet for VecDeque<Entity> {
    type SourceIter<'a> = Copied<alloc::collections::vec_deque::Iter<'a, Entity>>;

    const SINGLE_ENTITY: bool = false;

    fn with_hint(size_hint: usize) -> Self {
        VecDeque::with_capacity(size_hint)
    }

    fn insert(&mut self, entity: Entity) -> bool {
        if self.contains(&entity) {
            false
        } else {
            VecDeque::push_back(self, entity);
            true
        }
    }

    fn remove(&mut self, entity: Entity) -> bool {
        let mut iter = VecDeque::iter(self);
        match iter.position(|e| *e == entity) {
            Some(index) => {
                VecDeque::remove(self, index);
                true
            }
            None => false,
        }
    }

    fn clear(&mut self) {
        VecDeque::clear(self);
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        VecDeque::iter(self).copied()
    }

    fn len(&self) -> usize {
        VecDeque::len(self)
    }

    fn is_empty(&self) -> bool {
        VecDeque::is_empty(self)
    }

    fn reserve(&mut self, additional: usize) {
        VecDeque::reserve(self, additional)
    }

    fn shrink_to_fit(&mut self) {
        VecDeque::shrink_to_fit(self)
    }
}

impl OrderedRelationshipSourceSet for VecDeque<Entity> {
    fn insert_at(&mut self, index: usize, entity: Entity) {
        let index = index.min(VecDeque::len(self));
        VecDeque::insert(self, index, entity);
    }

    fn remove_at(&mut self, index: usize) -> Option<Entity> {
        VecDeque::remove(self, index)
    }

    fn insert_at_unstable(&mut self, index: usize, entity: Entity) {
        let len = VecDeque::len(self);
        VecDeque::push_back(self, entity);
        if index < len {
            self.swap(index, len);
        }
    }

    fn remove_at_unstable(&mut self, index: usize) -> Option<Entity> {
        VecDeque::swap_remove_back(self, index)
    }

    fn place_most_recent(&mut self, index: usize) {
        if let Some(entity) = self.pop_back() {
            let index = index.min(VecDeque::len(self));
            VecDeque::insert(self, index, entity);
        }
    }

    fn place(&mut self, entity: Entity, index: usize) {
        let mut slice = VecDeque::iter(self);
        if let Some(current) = slice.position(|e| *e == entity) {
            VecDeque::remove(self, current);
            let index = index.min(VecDeque::len(self));
            VecDeque::insert(self, index, entity);
        };
    }

    fn push_front(&mut self, entity: Entity) {
        VecDeque::push_front(self, entity);
    }

    fn push_back(&mut self, entity: Entity) {
        VecDeque::push_back(self, entity);
    }

    fn pop_front(&mut self) -> Option<Entity> {
        VecDeque::pop_front(self)
    }

    fn pop_back(&mut self) -> Option<Entity> {
        VecDeque::pop_back(self)
    }
}

impl<const N: usize> RelationshipSourceSet for SmallVec<Entity, N> {
    type SourceIter<'a> = Copied<core::slice::Iter<'a, Entity>>;

    const SINGLE_ENTITY: bool = false;

    fn with_hint(size_hint: usize) -> Self {
        SmallVec::with_capacity(size_hint)
    }

    fn insert(&mut self, entity: Entity) -> bool {
        if self.contains(&entity) {
            false
        } else {
            SmallVec::push(self, entity);
            true
        }
    }

    fn remove(&mut self, entity: Entity) -> bool {
        let mut iter = self.as_slice().iter();
        match iter.position(|e| *e == entity) {
            Some(index) => {
                SmallVec::remove(self, index);
                true
            }
            None => false,
        }
    }

    fn clear(&mut self) {
        SmallVec::clear(self);
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        self.as_slice().iter().copied()
    }

    fn len(&self) -> usize {
        SmallVec::len(self)
    }

    fn is_empty(&self) -> bool {
        SmallVec::is_empty(self)
    }

    fn reserve(&mut self, additional: usize) {
        SmallVec::reserve(self, additional)
    }

    fn shrink_to_fit(&mut self) {
        SmallVec::shrink_to_fit(self)
    }
}

impl<const N: usize> OrderedRelationshipSourceSet for SmallVec<Entity, N> {
    fn insert_at(&mut self, index: usize, entity: Entity) {
        let index = index.min(SmallVec::len(self));
        SmallVec::insert(self, index, entity);
    }

    fn remove_at(&mut self, index: usize) -> Option<Entity> {
        let in_bound = index < SmallVec::len(self);
        in_bound.then(|| SmallVec::remove(self, index))
    }

    fn insert_at_unstable(&mut self, index: usize, entity: Entity) {
        let len = SmallVec::len(self);
        SmallVec::push(self, entity);
        if index < len {
            self.swap(index, len);
        }
    }

    fn remove_at_unstable(&mut self, index: usize) -> Option<Entity> {
        if index < SmallVec::len(self) {
            Some(self.swap_remove(index))
        } else {
            None
        }
    }

    fn place_most_recent(&mut self, index: usize) {
        if let Some(entity) = self.pop() {
            let index = index.min(SmallVec::len(self));
            SmallVec::insert(self, index, entity);
        }
    }

    fn place(&mut self, entity: Entity, index: usize) {
        let mut slice = self.as_slice().iter();
        if let Some(current) = slice.position(|e| *e == entity) {
            SmallVec::remove(self, current);
            let index = index.min(SmallVec::len(self));
            SmallVec::insert(self, index, entity);
        };
    }

    fn push_front(&mut self, entity: Entity) {
        SmallVec::insert(self, 0, entity);
    }

    fn push_back(&mut self, entity: Entity) {
        SmallVec::push(self, entity);
    }

    fn pop_front(&mut self) -> Option<Entity> {
        let contains = SmallVec::is_empty(self);
        contains.then(|| SmallVec::remove(self, 0))
    }

    fn pop_back(&mut self) -> Option<Entity> {
        SmallVec::pop(self)
    }
}

// -----------------------------------------------------------------------------
// Set

impl RelationshipSourceSet for BTreeSet<Entity> {
    type SourceIter<'a> = Copied<alloc::collections::btree_set::Iter<'a, Entity>>;

    const SINGLE_ENTITY: bool = false;

    fn with_hint(_: usize) -> Self {
        BTreeSet::new()
    }

    fn insert(&mut self, entity: Entity) -> bool {
        BTreeSet::insert(self, entity)
    }

    fn remove(&mut self, entity: Entity) -> bool {
        BTreeSet::remove(self, &entity)
    }

    fn clear(&mut self) {
        BTreeSet::clear(self);
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        BTreeSet::iter(self).copied()
    }

    fn len(&self) -> usize {
        BTreeSet::len(self)
    }

    fn is_empty(&self) -> bool {
        BTreeSet::is_empty(self)
    }

    fn reserve(&mut self, _: usize) {}

    fn shrink_to_fit(&mut self) {}
}

impl RelationshipSourceSet for EntityHashSet {
    type SourceIter<'a> = Copied<crate::entity::hash_set::Iter<'a, Entity>>;

    const SINGLE_ENTITY: bool = false;

    fn with_hint(size_hint: usize) -> Self {
        EntityHashSet::with_capacity(size_hint)
    }

    fn insert(&mut self, entity: Entity) -> bool {
        EntityHashSet::insert(self, entity)
    }

    fn remove(&mut self, entity: Entity) -> bool {
        EntityHashSet::remove(self, &entity)
    }

    fn clear(&mut self) {
        EntityHashSet::clear(self);
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        EntityHashSet::iter(self).copied()
    }

    fn len(&self) -> usize {
        EntityHashSet::len(self)
    }

    fn is_empty(&self) -> bool {
        EntityHashSet::is_empty(self)
    }

    fn reserve(&mut self, additional: usize) {
        EntityHashSet::reserve(self, additional);
    }

    fn shrink_to_fit(&mut self) {
        EntityHashSet::shrink_to_fit(self);
    }
}

impl RelationshipSourceSet for EntityIndexSet {
    type SourceIter<'a> = Copied<crate::entity::index_set::Iter<'a, Entity>>;

    const SINGLE_ENTITY: bool = false;

    fn with_hint(size_hint: usize) -> Self {
        EntityIndexSet::with_capacity(size_hint)
    }

    fn insert(&mut self, entity: Entity) -> bool {
        EntityIndexSet::insert(self, entity)
    }

    fn remove(&mut self, entity: Entity) -> bool {
        EntityIndexSet::shift_remove(self, &entity)
    }

    fn clear(&mut self) {
        EntityIndexSet::clear(self);
    }

    fn iter(&self) -> Self::SourceIter<'_> {
        EntityIndexSet::iter(self).copied()
    }

    fn len(&self) -> usize {
        EntityIndexSet::len(self)
    }

    fn is_empty(&self) -> bool {
        EntityIndexSet::is_empty(self)
    }

    fn reserve(&mut self, additional: usize) {
        EntityIndexSet::reserve(self, additional);
    }

    fn shrink_to_fit(&mut self) {
        EntityIndexSet::shrink_to_fit(self);
    }
}
