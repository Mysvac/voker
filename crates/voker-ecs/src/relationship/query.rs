use alloc::collections::VecDeque;

use voker_utils::vec::SmallVec;

use super::{Relationship, RelationshipTarget, SourceIter};
use crate::entity::Entity;
use crate::query::{Query, QueryData, QueryFilter};

// -----------------------------------------------------------------------------
// Query helpers

impl<'w, 's, D: QueryData, F: QueryFilter> Query<'w, 's, D, F> {
    /// Returns the target entity of the `R` relationship component on `entity`.
    ///
    /// Returns `None` if `entity` does not have the `R` component.
    pub fn related<R: Relationship>(&'w self, entity: Entity) -> Option<Entity>
    where
        D::ReadOnly: QueryData<Item<'w> = &'w R>,
    {
        self.get(entity).map(|r| r.related_target()).ok()
    }

    /// Iterates all source entities stored in the `S` cache component on `entity`.
    ///
    /// Returns an empty iterator if `entity` has no `S` component.
    pub fn relationship_sources<S: RelationshipTarget>(
        &'w self,
        entity: Entity,
    ) -> impl Iterator<Item = Entity> + 'w
    where
        D::ReadOnly: QueryData<Item<'w> = &'w S>,
    {
        self.get(entity).into_iter().flat_map(RelationshipTarget::iter)
    }

    /// Walks the `R` relationship chain upward and returns the first ancestor that has no `R`
    /// component (the root of the hierarchy).
    ///
    /// # Warning
    ///
    /// This recurses infinitely on cyclic graphs. Only use on acyclic trees.
    pub fn root_ancestor<R: Relationship>(&'w self, entity: Entity) -> Entity
    where
        D::ReadOnly: QueryData<Item<'w> = &'w R>,
    {
        match self.get(entity) {
            Ok(r) => self.root_ancestor(r.related_target()),
            Err(_) => entity,
        }
    }

    /// Iterates all leaf entities in the `S` sub-tree rooted at `entity`.
    ///
    /// An entity is a leaf if it either has no `S` component or its `S` source set is empty.
    ///
    /// Traversal is depth-first. See [`Query::iter_descendants_depth_first`] for traversal order.
    ///
    /// # Warning
    ///
    /// Cyclic graphs cause infinite iteration. Only use on acyclic trees.
    pub fn iter_leaves<S: RelationshipTarget>(
        &'w self,
        entity: Entity,
    ) -> impl Iterator<Item = Entity> + use<'w, 's, D, F, S>
    where
        D::ReadOnly: QueryData<Item<'w> = &'w S>,
        SourceIter<'w, S>: DoubleEndedIterator,
    {
        self.iter_descendants_depth_first(entity)
            .filter(|&e| self.get(e).map(|sources| sources.is_empty()).unwrap_or(true))
    }

    /// Iterates all siblings of `entity` — that is, other entities that share the same `R`
    /// relationship target.
    ///
    /// `entity` itself is excluded from the result.
    ///
    /// This method requires the query to return `(Option<&R>, Option<&R::RelationshipTarget>)`,
    /// covering both sides of the relationship:
    ///
    /// ```ignore
    /// fn system(mut q: Query<(Option<&ChildOf>, Option<&Children>)>) {
    ///     for sibling in q.iter_siblings::<ChildOf>(entity) { /* ... */ }
    /// }
    /// ```
    pub fn iter_siblings<R: Relationship>(
        &'w self,
        entity: Entity,
    ) -> impl Iterator<Item = Entity> + use<'w, 's, D, F, R>
    where
        D::ReadOnly: QueryData<Item<'w> = (Option<&'w R>, Option<&'w R::RelationshipTarget>)>,
    {
        self.get(entity)
            .ok()
            .and_then(|(maybe_rel, _)| maybe_rel.map(|r| r.related_target()))
            .and_then(|parent| self.get(parent).ok())
            .and_then(|(_, maybe_target)| maybe_target)
            .into_iter()
            .flat_map(move |sources| sources.iter().filter(move |s| *s != entity))
    }

    /// Returns a breadth-first iterator over all descendants in the `S` hierarchy rooted at
    /// `entity`.
    ///
    /// See [`DescendantIter`] for the concrete iterator type.
    ///
    /// # Warning
    ///
    /// Cyclic graphs cause infinite iteration. Only use on acyclic trees.
    pub fn iter_descendants<S: RelationshipTarget>(
        &'w self,
        entity: Entity,
    ) -> DescendantIter<'w, 's, D, F, S>
    where
        D::ReadOnly: QueryData<Item<'w> = &'w S>,
    {
        DescendantIter::new(self, entity)
    }

    /// Returns a depth-first iterator over all descendants in the `S` hierarchy rooted at
    /// `entity`.
    ///
    /// See [`DescendantDepthFirstIter`] for the concrete iterator type.
    ///
    /// # Warning
    ///
    /// Cyclic graphs cause infinite iteration. Only use on acyclic trees.
    pub fn iter_descendants_depth_first<S: RelationshipTarget>(
        &'w self,
        entity: Entity,
    ) -> DescendantDepthFirstIter<'w, 's, D, F, S>
    where
        D::ReadOnly: QueryData<Item<'w> = &'w S>,
        SourceIter<'w, S>: DoubleEndedIterator,
    {
        DescendantDepthFirstIter::new(self, entity)
    }

    /// Returns an iterator over the `R` ancestors of `entity`, from immediate parent to root.
    ///
    /// See [`AncestorIter`] for the concrete iterator type.
    ///
    /// # Warning
    ///
    /// Cyclic graphs cause infinite iteration. Only use on acyclic trees.
    pub fn iter_ancestors<R: Relationship>(
        &'w self,
        entity: Entity,
    ) -> AncestorIter<'w, 's, D, F, R>
    where
        D::ReadOnly: QueryData<Item<'w> = &'w R>,
    {
        AncestorIter::new(self, entity)
    }
}

// -----------------------------------------------------------------------------
// DescendantIter

/// Breadth-first iterator over all descendants of an entity in an `S` relationship hierarchy.
///
/// Obtained via [`Query::iter_descendants`].
pub struct DescendantIter<'w, 's, D: QueryData, F: QueryFilter, S: RelationshipTarget>
where
    D::ReadOnly: QueryData<Item<'w> = &'w S>,
{
    query: &'w Query<'w, 's, D, F>,
    queue: VecDeque<Entity>,
}

impl<'w, 's, D: QueryData, F: QueryFilter, S: RelationshipTarget> DescendantIter<'w, 's, D, F, S>
where
    D::ReadOnly: QueryData<Item<'w> = &'w S>,
{
    /// Creates a new [`DescendantIter`] rooted at `entity`.
    pub fn new(query: &'w Query<'w, 's, D, F>, entity: Entity) -> Self {
        DescendantIter {
            query,
            queue: query
                .get(entity)
                .into_iter()
                .flat_map(RelationshipTarget::iter)
                .collect(),
        }
    }
}

impl<'w, 's, D: QueryData, F: QueryFilter, S: RelationshipTarget> Iterator
    for DescendantIter<'w, 's, D, F, S>
where
    D::ReadOnly: QueryData<Item<'w> = &'w S>,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        let entity = self.queue.pop_front()?;
        if let Ok(sources) = self.query.get(entity) {
            self.queue.extend(sources.iter());
        }
        Some(entity)
    }
}

// -----------------------------------------------------------------------------
// DescendantDepthFirstIter

/// Depth-first iterator over all descendants of an entity in an `S` relationship hierarchy.
///
/// Obtained via [`Query::iter_descendants_depth_first`].
///
/// Requires [`SourceIter`]`<'w, S>: DoubleEndedIterator` because children are pushed onto a
/// stack in reverse order so the first child is visited first.
pub struct DescendantDepthFirstIter<'w, 's, D, F, S>
where
    D: QueryData,
    F: QueryFilter,
    S: RelationshipTarget,
    D::ReadOnly: QueryData<Item<'w> = &'w S>,
    SourceIter<'w, S>: DoubleEndedIterator,
{
    query: &'w Query<'w, 's, D, F>,
    stack: SmallVec<Entity, 8>,
}

impl<'w, 's, D, F, S> DescendantDepthFirstIter<'w, 's, D, F, S>
where
    D: QueryData,
    F: QueryFilter,
    S: RelationshipTarget,
    D::ReadOnly: QueryData<Item<'w> = &'w S>,
    SourceIter<'w, S>: DoubleEndedIterator,
{
    /// Creates a new [`DescendantDepthFirstIter`] rooted at `entity`.
    pub fn new(query: &'w Query<'w, 's, D, F>, entity: Entity) -> Self {
        DescendantDepthFirstIter {
            query,
            stack: query
                .get(entity)
                .map_or(SmallVec::new(), |sources| sources.iter().rev().collect()),
        }
    }
}

impl<'w, 's, D, F, S> Iterator for DescendantDepthFirstIter<'w, 's, D, F, S>
where
    D: QueryData,
    F: QueryFilter,
    S: RelationshipTarget,
    D::ReadOnly: QueryData<Item<'w> = &'w S>,
    SourceIter<'w, S>: DoubleEndedIterator,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        let entity = self.stack.pop()?;
        if let Ok(sources) = self.query.get(entity) {
            self.stack.extend(sources.iter().rev());
        }
        Some(entity)
    }
}

// -----------------------------------------------------------------------------
// AncestorIter

/// Iterator over the `R` ancestors of an entity, yielding each parent in order from immediate
/// parent up to the root.
///
/// Obtained via [`Query::iter_ancestors`].
pub struct AncestorIter<'w, 's, D, F, R>
where
    D: QueryData,
    F: QueryFilter,
    R: Relationship,
    D::ReadOnly: QueryData<Item<'w> = &'w R>,
{
    query: &'w Query<'w, 's, D, F>,
    next: Option<Entity>,
}

impl<'w, 's, D, F, R> AncestorIter<'w, 's, D, F, R>
where
    D: QueryData,
    F: QueryFilter,
    R: Relationship,
    D::ReadOnly: QueryData<Item<'w> = &'w R>,
{
    /// Creates a new [`AncestorIter`] starting from `entity`.
    ///
    /// The iterator begins at `entity`'s immediate parent (not at `entity` itself).
    pub fn new(query: &'w Query<'w, 's, D, F>, entity: Entity) -> Self {
        AncestorIter {
            query,
            next: Some(entity),
        }
    }
}

impl<'w, 's, D, F, R> Iterator for AncestorIter<'w, 's, D, F, R>
where
    D: QueryData,
    F: QueryFilter,
    R: Relationship,
    D::ReadOnly: QueryData<Item<'w> = &'w R>,
{
    type Item = Entity;

    fn next(&mut self) -> Option<Self::Item> {
        self.next = self.query.get(self.next?).ok().map(|r| r.related_target());
        self.next
    }
}
