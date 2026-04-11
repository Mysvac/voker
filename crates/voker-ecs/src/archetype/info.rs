use alloc::vec::Vec;
use core::fmt::Debug;

use voker_utils::hash::SparseHashMap;

use crate::archetype::{ArcheId, ArcheRow};
use crate::bundle::BundleId;
use crate::component::{ComponentHook, HookContext};
use crate::component::{ComponentId, Components};
use crate::entity::{Entity, MovedEntityRow};
use crate::storage::TableId;
use crate::utils::DebugLocation;
use crate::world::DeferredWorld;

type HookItem = (ComponentId, ComponentHook);

// -----------------------------------------------------------------------------
// Archetype

/// A collection of entities that share the exact same set of component types.
///
/// An `Archetype` represents a unique combination of component types in the ECS.
/// All entities within the same archetype have identical component sets, enabling
/// efficient filtering and iteration with specific component combinations.
///
/// # Storage Strategy
///
/// `ComponentIds` are split into two categories for performance optimization:
/// `[dense_components][sparse_components]`, both component lists are kept sorted
/// to enable O(log n) lookups via binary search.
///
/// # Entity Management
/// The archetype maintains a contiguous array of entities, where the index
/// (`ArcheRow`) serves as a stable identifier for component data locations.
///
/// When entities are removed, swap-remove operation are used to maintain contiguity,
/// with moved entities tracked for reference updates.
///
/// # Query Filtering
///
/// The ECS query system employs a two-level filtering: *Archetype Filtering*
/// and *Entity Filtering*.
///
/// The first filtering pass operates at the archetype level, selecting entire archetypes
/// based on component requirements:
/// - **Required components (`with`)**: All must be present in the archetype
/// - **Excluded components (`without`)**: None may be present in the archetype
///
/// This pass quickly eliminates large groups of entities that cannot possibly match
/// the query, without examining individual entities.
///
/// # Archetype and Table
///
/// Due to the existence of sparse components, the relationship between `Archetype`
/// and `Table` is **N:1**.
///
/// For queries that do not involve any sparse components, the `Archetype` can be
/// replaced directly by the `Table` for iteration purposes.
///
/// However, for operations such as entity spawning or component insertion/removal,
/// the `Archetype` contains complete information and cannot be replaced by the `Table`.
pub struct Archetype {
    id: ArcheId,
    // Backing table for dense components, optimize query.
    table_id: TableId,
    // Number of components stored in the table region.
    dense_len: usize,
    // - `[..dense_len]` are stored in tables (sorted).
    // - `[dense_len..]` are stored in sparse maps (sorted).
    components: &'static [ComponentId],
    // Maps archetype rows to entities.
    entities: Vec<Entity>,
    // Cached component hooks.
    on_add: &'static [HookItem],
    on_clone: &'static [HookItem],
    on_insert: &'static [HookItem],
    on_remove: &'static [HookItem],
    on_discard: &'static [HookItem],
    on_despawn: &'static [HookItem],
    // Cached archetype transitions for bundle insertion/removal.
    after_insert: SparseHashMap<BundleId, ArcheId>,
    after_remove: SparseHashMap<BundleId, ArcheId>,
}

// -----------------------------------------------------------------------------
// Private

impl Archetype {
    /// Create a new `Archetype` from given information.
    pub(super) fn new(
        arche_id: ArcheId,
        table_id: TableId,
        dense_len: usize,
        idents: &'static [ComponentId],
        components: &Components,
    ) -> Self {
        use crate::utils::SlicePool;

        debug_assert!(idents[..dense_len].is_sorted());
        debug_assert!(idents[dense_len..].is_sorted());

        let mut on_add = Vec::new();
        let mut on_clone = Vec::new();
        let mut on_insert = Vec::new();
        let mut on_remove = Vec::new();
        let mut on_discard = Vec::new();
        let mut on_despawn = Vec::new();

        #[rustfmt::skip] // For compact format.
        idents.iter().for_each(|&id| unsafe {
            let info = components.get_unchecked(id);
            if let Some(hk) = info.on_add() { on_add.push((id, hk)); }
            if let Some(hk) = info.on_clone() { on_clone.push((id, hk)); }
            if let Some(hk) = info.on_insert() { on_insert.push((id, hk)); }
            if let Some(hk) = info.on_remove() { on_remove.push((id, hk)); }
            if let Some(hk) = info.on_discard() { on_discard.push((id, hk)); }
            if let Some(hk) = info.on_despawn() { on_despawn.push((id, hk)); }
        });

        let on_add = SlicePool::component_hook(&on_add);
        let on_clone = SlicePool::component_hook(&on_clone);
        let on_insert = SlicePool::component_hook(&on_insert);
        let on_remove = SlicePool::component_hook(&on_remove);
        let on_discard = SlicePool::component_hook(&on_discard);
        let on_despawn = SlicePool::component_hook(&on_despawn);

        Archetype {
            id: arche_id,
            table_id,
            dense_len,
            components: idents,
            on_add,
            on_clone,
            on_insert,
            on_remove,
            on_discard,
            on_despawn,
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
    /// The returned slice is laid out as
    /// `[dense_components][sparse_components]`, where each region is sorted.
    #[inline(always)]
    pub fn components(&self) -> &'static [ComponentId] {
        self.components
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
    pub fn dense_components(&self) -> &'static [ComponentId] {
        #[cfg(not(debug_assertions))]
        unsafe {
            // Disable boundary checking during release.
            ::core::hint::assert_unchecked(self.dense_len <= self.components.len());
        }

        &self.components[..self.dense_len]
    }

    /// Returns the list of sparse component types stored in maps.
    ///
    /// These components use map-based storage to optimize memory usage
    /// when components are infrequently present. The slice is guaranteed
    /// to be sorted and non-overlapping with dense components.
    #[inline(always)]
    pub fn sparse_components(&self) -> &'static [ComponentId] {
        #[cfg(not(debug_assertions))]
        unsafe {
            // Disable boundary checking during release.
            ::core::hint::assert_unchecked(self.dense_len <= self.components.len());
        }

        &self.components[self.dense_len..]
    }

    /// Checks if this archetype contains a specific component type.
    ///
    /// # Complexity
    /// - Time: O(log d + log s), where `d` is the number of dense components
    ///   and `s` is the number of sparse components.
    #[inline]
    pub fn contains_component(&self, id: ComponentId) -> bool {
        #[cold]
        #[inline(never)]
        fn large_contains(this: &Archetype, id: ComponentId) -> bool {
            this.dense_components().binary_search(&id).is_ok()
                || this.sparse_components().binary_search(&id).is_ok()
        }

        // ComponentId is easy to optimize with SIMD, and linear search
        // is faster when the data is less than 100 (release + O3).
        if self.components.len() < 200 {
            crate::utils::contains_component(id, self.components)
        } else {
            large_contains(self, id)
        }
    }

    /// Checks if this archetype contains a specific dense component type.
    ///
    /// # Complexity
    /// - Time: O(log n) where n is the number of dense components
    #[inline]
    pub fn contains_dense_component(&self, id: ComponentId) -> bool {
        #[cold]
        #[inline(never)]
        fn large_contains(this: &Archetype, id: ComponentId) -> bool {
            this.dense_components().binary_search(&id).is_ok()
        }

        // ComponentId is easy to optimize with SIMD, and linear search
        // is faster when the data is less than 100 (release + O3).
        if self.components.len() < 200 {
            crate::utils::contains_component(id, self.dense_components())
        } else {
            large_contains(self, id)
        }
    }

    /// Checks if this archetype contains a specific sparse component type.
    ///
    /// # Complexity
    /// - Time: O(log n) where n is the number of sparse components
    #[inline]
    pub fn contains_sparse_component(&self, id: ComponentId) -> bool {
        #[cold]
        #[inline(never)]
        fn large_contains(this: &Archetype, id: ComponentId) -> bool {
            this.sparse_components().binary_search(&id).is_ok()
        }

        // ComponentId is easy to optimize with SIMD, and linear search
        // is faster when the data is less than 100 (release + O3).
        if self.components.len() < 200 {
            crate::utils::contains_component(id, self.sparse_components())
        } else {
            large_contains(self, id)
        }
    }
}

// -----------------------------------------------------------------------------
// Entity Operation

impl Archetype {
    /// Finds the row index for a given entity using linear search.
    ///
    /// It can also be used to check whether a specified entity is included.
    ///
    /// Note: This is inefficient and should be avoided.
    ///
    /// # Complexity
    /// O(n) where n is the number of entities
    #[must_use]
    pub fn get_arche_row(&self, entity: Entity) -> Option<ArcheRow> {
        self.entities
            .iter()
            .position(|e| *e == entity)
            .map(|idx| ArcheRow(idx as u32))
    }

    /// Inserts a new entity into this archetype, reserving space at the end.
    ///
    /// # Safety
    /// - **Entity uniqueness**: The entity must not already exist in this archetype.
    /// - **Storage preparation**: The caller must ensure that component storage is
    ///   prepared for this entity before or immediately after insertion.
    #[must_use]
    pub unsafe fn alloc_row(&mut self, entity: Entity) -> ArcheRow {
        debug_assert!(!crate::utils::contains_entity(entity, &self.entities));
        // 0 < EntityId < u32::MAX
        let row = ArcheRow(self.entities.len() as u32);
        self.entities.push(entity);
        row
    }

    /// Removes an entity from this archetype using swap-remove semantics.
    ///
    /// # Returns
    /// `MovedEntityRow` - containing the moved entity and its new location.
    ///
    /// # Safety
    /// - **Row validity**: The provided `row` must be currently occupied by an entity.
    /// - **External reference updates**: The caller MUST update any external references
    ///   that pointed to the moved entity's old location.
    #[must_use]
    pub unsafe fn dealloc_row(&mut self, row: ArcheRow) -> MovedEntityRow {
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
// Bundle - Insertion & Removal

impl Archetype {
    /// Returns the cached target archetype after inserting a bundle.
    pub fn after_insert(&self, bundle: BundleId) -> Option<ArcheId> {
        self.after_insert.get(&bundle).copied()
    }

    /// Returns the cached target archetype after removing a bundle.
    pub fn after_remove(&self, bundle: BundleId) -> Option<ArcheId> {
        self.after_remove.get(&bundle).copied()
    }

    /// Caches the target archetype for an insert-bundle transition.
    pub fn set_after_insert(&mut self, bundle: BundleId, arche: ArcheId) {
        self.after_insert.insert(bundle, arche);
    }

    /// Caches the target archetype for a remove-bundle transition.
    pub fn set_after_remove(&mut self, bundle: BundleId, arche: ArcheId) {
        self.after_remove.insert(bundle, arche);
    }
}

// -----------------------------------------------------------------------------
// Component Hooks

impl Archetype {
    /// Returns cached hooks triggered when a component is added.
    #[inline]
    pub fn on_add_hooks(&self) -> &[HookItem] {
        self.on_add
    }

    /// Returns cached hooks triggered when a component is cloned.
    #[inline]
    pub fn on_clone_hooks(&self) -> &[HookItem] {
        self.on_clone
    }

    /// Returns cached hooks triggered when a component is inserted.
    #[inline]
    pub fn on_insert_hooks(&self) -> &[HookItem] {
        self.on_insert
    }

    /// Returns cached hooks triggered when a component is removed.
    #[inline]
    pub fn on_remove_hooks(&self) -> &[HookItem] {
        self.on_remove
    }

    /// Returns cached hooks triggered when a component is discarded.
    #[inline]
    pub fn on_discard_hooks(&self) -> &[HookItem] {
        self.on_discard
    }

    /// Returns cached hooks triggered when an entity is despawned.
    #[inline]
    pub fn on_despawn_hooks(&self) -> &[HookItem] {
        self.on_despawn
    }

    /// Triggers all `on_add` hooks in this archetype for the given entity.
    #[inline]
    #[track_caller]
    pub(crate) fn trigger_on_add(
        &self,
        entity: Entity,
        mut world: DeferredWorld,
        caller: DebugLocation,
    ) {
        self.on_add.iter().for_each(|&(id, hook)| {
            hook(world.reborrow(), HookContext { id, entity, caller });
        });
    }

    /// Triggers all `on_clone` hooks in this archetype for the given entity.
    #[inline]
    #[track_caller]
    pub(crate) fn trigger_on_clone(
        &self,
        entity: Entity,
        mut world: DeferredWorld,
        caller: DebugLocation,
    ) {
        self.on_clone.iter().for_each(|&(id, hook)| {
            hook(world.reborrow(), HookContext { id, entity, caller });
        });
    }

    /// Triggers all `on_insert` hooks in this archetype for the given entity.
    #[inline]
    #[track_caller]
    pub(crate) fn trigger_on_insert(
        &self,
        entity: Entity,
        mut world: DeferredWorld,
        caller: DebugLocation,
    ) {
        self.on_insert.iter().for_each(|&(id, hook)| {
            hook(world.reborrow(), HookContext { id, entity, caller });
        });
    }

    /// Triggers all `on_remove` hooks in this archetype for the given entity.
    #[inline]
    #[track_caller]
    pub(crate) fn trigger_on_remove(
        &self,
        entity: Entity,
        mut world: DeferredWorld,
        caller: DebugLocation,
    ) {
        self.on_remove.iter().for_each(|&(id, hook)| {
            hook(world.reborrow(), HookContext { id, entity, caller });
        });
    }

    /// Triggers all `on_discard` hooks in this archetype for the given entity.
    #[inline]
    pub(crate) fn trigger_on_discard(
        &self,
        entity: Entity,
        mut world: DeferredWorld,
        caller: DebugLocation,
    ) {
        self.on_discard.iter().for_each(|&(id, hook)| {
            hook(world.reborrow(), HookContext { id, entity, caller });
        });
    }

    /// Triggers all `on_despawn` hooks in this archetype for the given entity.
    #[inline]
    pub(crate) fn trigger_on_despawn(
        &self,
        entity: Entity,
        mut world: DeferredWorld,
        caller: DebugLocation,
    ) {
        self.on_despawn.iter().for_each(|&(id, hook)| {
            hook(world.reborrow(), HookContext { id, entity, caller });
        });
    }
}
