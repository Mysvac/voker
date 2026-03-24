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
    // - `[0..dense_len]` are stored in Tables, sorted.
    // - `[dense_len..]` are stored in Maps, sorted.
    // We use Arc to reduce memory allocation overhead.
    components: Arc<[ComponentId]>,
    /// Maps archetype rows to their corresponding entities.
    /// The vector index = `ArcheRow`, value = `Entity`.
    /// Maintained in contiguous order for O(1) entity lookup by row.
    entities: Vec<Entity>,
    after_insert: SparseHashMap<BundleId, ArcheId>,
    after_remove: SparseHashMap<BundleId, ArcheId>,
}

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
    /// # Safety
    /// - valid arche_id
    /// - table_id matched components
    /// - `components[..dense_len]` are stored in Tables, sorted.
    /// - `components[dense_len..]` are stored in Maps, sorted.
    pub(super) unsafe fn new(
        arche_id: ArcheId,
        table_id: TableId,
        dense_len: usize,
        components: Arc<[ComponentId]>,
    ) -> Self {
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

    /// Checks if this archetype contains a specific component type.
    ///
    /// # Complexity
    /// - Time: O(log n) where n is the total number of component types
    /// - Space: O(1)
    pub fn contains_component(&self, id: ComponentId) -> bool {
        self.contains_dense_component(id) || self.contains_sparse_component(id)
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

    /// Checks whether this archetype matches the given component requirements.
    ///
    /// This is the primary method for **archetype-level filtering** in the query system.
    /// It quickly determines if an archetype qualifies for further processing without
    /// examining individual entities.
    ///
    /// # Parameters
    /// - `with` - Component types that must be present
    ///   (order doesn't matter, allow duplicates)
    /// - `without` - Component types that must be absent
    ///   (order doesn't matter, allow duplicates)
    ///
    /// # Complexity
    /// - Time: O(m * log n) where m = len(with) + len(without), n = total components
    /// - Space: O(1)
    ///
    /// For better performance with sorted inputs, see [`matches_sorted`](Self::matches_sorted).
    pub fn matches(&self, with: &[ComponentId], without: &[ComponentId]) -> bool {
        with.iter().all(|id| self.contains_component(*id))
            && !without.iter().any(|id| self.contains_component(*id))
    }

    /// Fast archetype matching requiring sorted input slices.
    ///
    /// This optimized variant is designed for high-throughput query processing,
    /// particularly when filtering many archetypes with the same query.
    ///
    /// # Parameters
    /// - `with` - Component types that must be present
    ///   (**must be sorted and deduplicated**)
    /// - `without` - Component types that must be absent
    ///   (**must be sorted and deduplicated**)
    ///
    /// # Requirements
    /// Both input slices **MUST** be sorted in ascending order. Duplicate entries
    /// are **not** allowed. Violating the requirement leads to unspecified results
    /// (but not memory unsafety).
    ///
    /// # Complexity
    /// - Time: O(min(m + n, m * log n)) where m = len(with) + len(without), n = total components
    /// - Space: O(1)
    pub fn matches_sorted(&self, with: &[ComponentId], without: &[ComponentId]) -> bool {
        fn jump_search(id: ComponentId, slice: &[ComponentId]) -> Result<usize, usize> {
            let mut index = 0usize;
            let len = slice.len();

            loop {
                if index == len || slice[index] > id {
                    return Err(index);
                }
                if slice[index] == id {
                    return Ok(index);
                }

                let mut step = 1usize;
                loop {
                    let offset = index + step;
                    if offset < len && slice[offset] <= id {
                        step <<= 1;
                    } else {
                        break;
                    }
                }
                // index + (step >> 1) < len
                // index + min(step >> 1, 1) <= len
                index += core::cmp::max(step >> 1, 1);
            }
        }
        {
            // with
            let mut dense = self.dense_components();
            let mut sparse = self.sparse_components();
            let result = with.iter().all(|&id| {
                // `with` has been sorted and deduplicated, the `[..=idx]` can be skipped.
                // we can skip `[idx]` because it's `==` specific id.
                if let Ok(idx) = jump_search(id, dense) {
                    dense = &dense[(idx + 1)..];
                    return true;
                }
                if let Ok(idx) = jump_search(id, sparse) {
                    sparse = &sparse[(idx + 1)..];
                    return true;
                }
                false
            });
            if !result {
                return false;
            }
        }
        {
            // without
            let mut dense = self.dense_components();
            let mut sparse = self.sparse_components();
            // `without` has been sorted and deduplicated, the `[..idx]` can be skipped.
            // cannot skip `[idx]` because it's `>` specific id (or the end of slice).
            without.iter().all(|&id| {
                if let Err(idx) = jump_search(id, dense) {
                    dense = &dense[idx..];
                } else {
                    return false;
                }
                if let Err(idx) = jump_search(id, sparse) {
                    sparse = &sparse[idx..];
                } else {
                    return false;
                }
                true
            })
        }
    }

    /// Returns a slice of all entities in this archetype.
    ///
    /// The entities are stored in the order of their archetype rows,
    /// which is also the iteration order for component data.
    #[inline(always)]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
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
    pub unsafe fn insert_entity(&mut self, entity: Entity) -> ArcheRow {
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
