use alloc::vec::Vec;
use core::fmt::Debug;
use voker_utils::hash::SparseHashMap;

use voker_os::sync::Arc;

use crate::archetype::{ArcheId, ArcheRow};
use crate::bundle::BundleId;
use crate::component::ComponentId;
use crate::entity::{Entity, MovedEntityRow};
use crate::storage::TableId;

// -----------------------------------------------------------------------------
// Archetype

/// A collection of entities that share the exact same set of component types.
///
/// # Overview
/// An `Archetype` represents a unique combination of component types in the ECS.
/// All entities within the same archetype have identical component sets, enabling:
/// - Efficient iteration over entities with specific component combinations
/// - Optimal memory layout through columnar storage
/// - Fast component access via table lookups
///
/// # Storage Strategy
/// ComponentIds are split into two categories for performance optimization:
/// - **Dense components** (`[..dense_len]`): Stored in contiguous tables for
///   cache-efficient iteration
/// - **Sparse components** (`[dense_len..]`): Stored in maps for memory efficiency
///   when components are rarely present
///
/// Both component lists are kept sorted to enable O(log n) lookups via binary search.
///
/// # Entity Management
/// The archetype maintains a contiguous array of entities, where the index
/// (`ArcheRow`) serves as a stable identifier for component data locations.
///
/// When entities are removed, swap-remove semantics are used to maintain
/// contiguity, with moved entities tracked for reference updates.
///
/// # Query Filtering Architecture
/// The ECS query system employs a two-level filtering strategy for optimal performance:
///
/// ## Level 1: Archetype Filtering (Coarse-grained)
/// The first filtering pass operates at the archetype level, selecting entire archetypes
/// based on component requirements:
/// - **Required components (`with`)**: All must be present in the archetype
/// - **Excluded components (`without`)**: None may be present in the archetype
///
/// This pass quickly eliminates large groups of entities that cannot possibly match
/// the query, without examining individual entities.
///
/// ## Level 2: Entity Filtering (Fine-grained)
/// After archetype filtering, individual entities within matching archetypes are
/// evaluated against additional query conditions (e.g., component value constraints,
/// relationship conditions, or custom predicates).
///
/// ## Optimization: Dense-Only Queries
/// A special optimization applies when queries involve **only dense components**:
/// - For such queries, matching archetypes correspond exactly to entire tables
/// - Instead of iterating through archetype entities (which point to scattered rows),
///   we can iterate directly over **table rows** for maximum cache efficiency
/// - This yields significant performance gains as table storage is fully contiguous
pub struct Archetype {
    // A unique identifier for a Archetype.
    // Also the index in the archetypes array
    id: ArcheId,
    // An archetype represents a unique combination of components.
    // Since its set of components is fixed, we cache the table ID
    // to optimize operations.
    table_id: TableId,
    // The number of components stored in the table.
    // Due to struct alignment, `usize` is equivalent to `u32`.
    dense_len: usize,
    // - `[..dense_len]` are stored in Tables, sorted.
    // - `[dense_len..]` are stored in Maps, sorted.
    // We use Arc to reduce memory allocation overhead.
    components: Arc<[ComponentId]>,
    // Maps archetype rows to their corresponding entities.
    // The vector index = `ArcheRow`, value = `Entity`.
    // Maintained in contiguous order for O(1) entity lookup by row.
    entities: Vec<Entity>,
    // Optimize component insertion and removal.
    after_insert: SparseHashMap<BundleId, ArcheId>,
    after_remove: SparseHashMap<BundleId, ArcheId>,
}

// -----------------------------------------------------------------------------
// Private

impl Archetype {
    /// # Requirement
    /// - valid arche_id
    /// - table_id matched components
    /// - `components[..dense_len]` are stored in Tables, sorted.
    /// - `components[dense_len..]` are stored in Maps, sorted.
    pub(super) fn new(
        arche_id: ArcheId,
        table_id: TableId,
        dense_len: usize,
        components: Arc<[ComponentId]>,
    ) -> Self {
        debug_assert!(components[..dense_len].is_sorted());
        debug_assert!(components[dense_len..].is_sorted());
        Archetype {
            id: arche_id,
            table_id,
            dense_len,
            components,
            entities: Vec::new(),
            after_insert: SparseHashMap::new(),
            after_remove: SparseHashMap::new(),
        }
    }
}

// -----------------------------------------------------------------------------
// Basic

impl Debug for Archetype {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Archetype")
            .field("id", &self.id)
            .field("table_id", &self.table_id)
            .field("components", &self.components)
            .field("entities", &self.entities)
            .finish()
    }
}

impl Archetype {
    /// Returns the unique identifier of this archetype.
    #[inline(always)]
    pub fn id(&self) -> ArcheId {
        self.id
    }

    /// Returns the table ID where dense components are stored.
    #[inline(always)]
    pub fn table_id(&self) -> TableId {
        self.table_id
    }

    /// Returns the complete list of component types in this archetype.
    ///
    /// The returned slice combines both dense and sparse components in sorted order.
    /// Similar to `[dense_components][sparse_components]`.
    #[inline(always)]
    pub fn components(&self) -> &[ComponentId] {
        &self.components
    }

    /// Checks if this archetype contains a specific component type.
    ///
    /// # Complexity
    /// - Time: O(log n) where n is the total number of component types
    /// - Space: O(1)
    pub fn contains_component(&self, id: ComponentId) -> bool {
        self.contains_dense_component(id) || self.contains_sparse_component(id)
    }

    /// Returns a slice of all entities in this archetype.
    ///
    /// The entities are stored in the order of their archetype rows,
    /// which is also the iteration order for component data.
    #[inline(always)]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }
}

// -----------------------------------------------------------------------------
// Filter

impl Archetype {
    /// Returns the list of dense component types stored in tables.
    ///
    /// These components benefit from cache-efficient iteration due to
    /// contiguous storage layout. The slice is guaranteed to be sorted.
    #[inline(always)]
    pub fn dense_components(&self) -> &[ComponentId] {
        &self.components[..self.dense_len]
    }

    /// Returns the list of sparse component types stored in maps.
    ///
    /// These components use map-based storage to optimize memory usage
    /// when components are infrequently present. The slice is guaranteed
    /// to be sorted and non-overlapping with dense components.
    #[inline(always)]
    pub fn sparse_components(&self) -> &[ComponentId] {
        &self.components[self.dense_len..]
    }

    /// Checks if this archetype contains a specific dense component type.
    ///
    /// # Complexity
    /// - Time: O(log n) where n is the number of dense components
    /// - Space: O(1)
    pub fn contains_dense_component(&self, id: ComponentId) -> bool {
        self.dense_components().binary_search(&id).is_ok()
    }

    /// Checks if this archetype contains a specific sparse component type.
    ///
    /// # Complexity
    /// - Time: O(log s) where s is the number of sparse components
    /// - Space: O(1)
    pub fn contains_sparse_component(&self, id: ComponentId) -> bool {
        self.sparse_components().binary_search(&id).is_ok()
    }
}

// -----------------------------------------------------------------------------
// Entity Operation

impl Archetype {
    /// Finds the row index for a given entity using linear search.
    ///
    /// # Complexity
    /// O(n) where n is the number of entities
    ///
    /// Note: This is inefficient and should be avoided.
    #[must_use]
    pub fn get_arche_row(&self, entity: Entity) -> Option<ArcheRow> {
        self.entities
            .iter()
            .position(|e| *e == entity)
            .map(|idx| ArcheRow(idx as u32))
    }

    /// Returns the entity at the specified archetype row.
    ///
    /// # Safety
    /// The provided `row` must be currently occupied by an entity.
    /// Calling with an invalid or empty row leads to undefined behavior.
    ///
    /// # Complexity
    /// - Time: O(1)
    /// - Space: O(1)
    #[must_use]
    pub unsafe fn entity_at(&mut self, row: ArcheRow) -> Entity {
        debug_assert!((row.0 as usize) < self.entities.len());
        unsafe { *self.entities.get_unchecked(row.0 as usize) }
    }

    /// Inserts a new entity into this archetype, reserving space at the end.
    ///
    /// This method adds an entity to the archetype, assigning it the next available
    /// archetype row. The entity's component data must be separately initialized
    /// in the appropriate storage locations (tables for dense components, maps for
    /// sparse components) before or after calling this method.
    ///
    /// # Safety
    /// This method is unsafe because it maintains critical invariants that must be
    /// upheld by the caller:
    ///
    /// - **Entity uniqueness**: The entity must not already exist in this archetype.
    ///   Duplicate entities would break the entity-to-row mapping, causing undefined
    ///   behavior when accessing components or iterating entities.
    ///
    /// - **Storage preparation**: The caller must ensure that component storage
    ///   (tables for dense components, maps for sparse components) is properly
    ///   prepared for this entity before or immediately after insertion. This
    ///   typically involves:
    ///   - Allocating space in the corresponding table for dense components
    ///   - Initializing map entries for sparse components
    ///   - Setting initial component values
    ///
    /// - **Exclusive access**: This method must be called with exclusive access to
    ///   the archetype (i.e., not while other operations are reading or writing
    ///   the entity list).
    ///
    /// # Complexity
    /// - Time: O(1)
    /// - Space: O(1)
    #[must_use]
    pub unsafe fn insert_entity(&mut self, entity: Entity) -> ArcheRow {
        // 0 < EntitIndex < u32::MAX
        let row = ArcheRow(self.entities.len() as u32);
        self.entities.push(entity);
        row
    }

    /// Removes an entity from this archetype using swap-remove semantics.
    ///
    /// This method removes the entity at the specified row and maintains contiguity
    /// of the entity array by moving the last entity into the vacated position
    /// (if the removed entity wasn't already the last one).
    ///
    /// # Returns
    /// - `Some(MovedEntity)` - If another entity was moved to fill the gap,
    ///   containing the moved entity and its original location (which is now
    ///   the row that needs updating in external references)
    /// - `None` - If the removed entity was the last one (no entity was moved)
    ///
    /// # Safety
    /// This method is unsafe because it modifies critical data structures and
    /// requires the caller to maintain invariants:
    ///
    /// - **Row validity**: The provided `row` must be currently occupied by an
    ///   entity. Calling with an invalid or empty row leads to undefined behavior.
    ///
    /// - **External reference updates**: If this method returns `MovedEntity`,
    ///   the caller MUST update any external references that pointed to the moved
    ///   entity's old location.
    ///
    /// - **Storage cleanup**: This method only removes the entity from the archetype's
    ///   entity list. The caller is responsible for cleaning up the entity's component
    ///   data from storage.
    ///
    /// # Complexity
    /// - Time: O(1)
    /// - Space: O(1)
    #[must_use]
    pub unsafe fn remove_entity(&mut self, row: ArcheRow) -> MovedEntityRow {
        debug_assert!((row.0 as usize) < self.entities.len());

        let last = self.entities.len() - 1;
        let dst = row.0 as usize;

        unsafe {
            if dst == last {
                self.entities.set_len(last);
                MovedEntityRow::in_arche(None, row)
            } else {
                let entity = *self.entities.get_unchecked(last);
                *self.entities.get_unchecked_mut(dst) = entity;
                self.entities.set_len(last);
                MovedEntityRow::in_arche(Some(entity), row)
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Bundle

impl Archetype {
    /// Obtain the new archetype id after inserting a Component.
    pub fn after_insert(&self, bundle: BundleId) -> Option<ArcheId> {
        self.after_insert.get(&bundle).copied()
    }

    /// Obtain the new archetype id after removing a Component.
    pub fn after_remove(&self, bundle: BundleId) -> Option<ArcheId> {
        self.after_remove.get(&bundle).copied()
    }

    /// Set a new archetype after inserting a Component.
    pub fn set_after_insert(&mut self, bundle: BundleId, arche: ArcheId) {
        self.after_insert.insert(bundle, arche);
    }

    /// Set a new archetype after removing a Component.
    pub fn set_after_remove(&mut self, bundle: BundleId, arche: ArcheId) {
        self.after_remove.insert(bundle, arche);
    }
}
