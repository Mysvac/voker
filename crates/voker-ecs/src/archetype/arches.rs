use alloc::vec::Vec;
use core::fmt::Debug;

use voker_os::sync::Arc;
use voker_utils::hash::HashMap;

use crate::archetype::{ArcheId, Archetype};
use crate::bundle::BundleId;
use crate::component::ComponentId;
use crate::storage::TableId;

// -----------------------------------------------------------------------------
// Archetypes

/// A collection of all archetypes in the ECS world.
///
/// # Overview
/// `Archetypes` serves as the central registry for all archetype instances,
/// providing efficient lookup and filtering capabilities for the ECS query system.
/// It maintains multiple indexing structures to support different access patterns:
///
/// - **Direct access**: By [`ArcheId`] (primary key)
/// - **Bundle-based**: Maps [`BundleId`] to the corresponding archetype
/// - **Precise matching**: Maps exact component sets to their archetype IDs
///
/// # Initial State
/// Always contains at least one archetype: the **empty archetype** (no components),
/// which serves as the starting point for all entities.
pub struct Archetypes {
    // Primary storage for all archetype instances.
    // Index corresponds directly to [`ArcheId`].
    arches: Vec<Archetype>,
    // Maps exact component sets to archetype IDs.
    mapper: HashMap<Arc<[ComponentId]>, ArcheId>,
    // Maps bundle IDs to archetype IDs, optimize entity spawn.
    bundles: Vec<Option<ArcheId>>,
}

// -----------------------------------------------------------------------------
// Methods

impl Debug for Archetypes {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(self.arches.as_slice(), f)
    }
}

impl Archetypes {
    /// Creates a new archetypes collection, initialized with the empty archetype.
    pub(crate) fn new() -> Self {
        let mut val = const {
            Archetypes {
                arches: Vec::new(),
                bundles: Vec::new(),
                mapper: HashMap::new(),
            }
        };

        let arche = Archetype::new(ArcheId::EMPTY, TableId::EMPTY, 0, Arc::new([]));
        val.arches.push(arche);
        val.mapper.insert(Arc::new([]), ArcheId::EMPTY);
        val.bundles.push(Some(ArcheId::EMPTY));

        val
    }

    /// Inserts a mapping from a bundle ID to an archetype ID.
    ///
    /// This mapping enables fast archetype lookup when spawning entities
    /// from known bundles.
    ///
    /// # Safety
    /// This method is unsafe because it modifies internal indexing structures
    /// and requires the caller to uphold the following invariants:
    ///
    /// - **Bundle validity**: The `bundle_id` must be valid and properly initialized
    ///   (i.e., corresponds to a registered bundle type).
    /// - **Archetype validity**: The `arche_id` must reference a valid, already-registered
    ///   archetype that exactly matches the component set of the bundle.
    /// - **No concurrent access**: This method may resize the bundle map; ensure no
    ///   other operations are concurrently reading or writing the bundle map.
    pub(crate) unsafe fn set_bundle_map(&mut self, bundle_id: BundleId, arche_id: ArcheId) {
        #[cold]
        #[inline(never)]
        fn resize_bundle_map(map: &mut Vec<Option<ArcheId>>, len: usize) {
            map.reserve(len - map.len());
            map.resize(map.capacity(), None);
        }

        let index = bundle_id.index();
        if index >= self.bundles.len() {
            resize_bundle_map(&mut self.bundles, index + 1);
        }
        unsafe {
            *self.bundles.get_unchecked_mut(index) = Some(arche_id);
        }
    }

    /// Registers a new archetype with the given component set.
    ///
    /// This method creates a new archetype and updates all indexing structures
    /// to make it discoverable through various lookup paths.
    ///
    /// # Safety
    /// This method is unsafe and requires the caller to ensure:
    ///
    /// - **Component validity**: All `ComponentId`s in `components` must be valid and
    ///   properly registered in the component registry.
    /// - **Uniqueness**: The exact component set must not already have an archetype
    ///   (no duplicates), unless intentionally creating a new archetype for the same
    ///   set (which would violate ECS invariants).
    /// - **Sorting**: The `components` slice must be sorted, as this is relied upon
    ///   for binary search operations in archetype methods.
    /// - **Bundle consistency**: If a bundle corresponds to this component set, its
    ///   mapping should be updated separately via [`insert_bundle_id`](Self::insert_bundle_id).
    pub(crate) unsafe fn register(
        &mut self,
        table_id: TableId,
        dense_len: usize,
        components: Arc<[ComponentId]>,
    ) -> ArcheId {
        let arche_id = ArcheId::new(self.arches.len() as u32);

        let arche = Archetype::new(arche_id, table_id, dense_len, components.clone());

        self.arches.push(arche);

        self.mapper.insert(components, arche_id);

        arche_id
    }
}

impl Archetypes {
    /// Returns the number of registered archetypes.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "len > 0")]
    pub fn len(&self) -> usize {
        self.arches.len()
    }

    /// Finds the archetype ID for an exact component set.
    #[inline]
    pub fn get_id(&self, components: &[ComponentId]) -> Option<ArcheId> {
        self.mapper.get(components).copied()
    }

    /// Returns the archetype ID associated with a specific bundle.
    #[inline]
    pub fn get_id_by_bundle(&self, id: BundleId) -> Option<ArcheId> {
        self.bundles.get(id.index()).and_then(|t| *t)
    }

    /// Returns a reference to the archetype with the given ID, if it exists.
    #[inline]
    pub fn get(&self, id: ArcheId) -> Option<&Archetype> {
        self.arches.get(id.index())
    }

    /// Returns a mutable reference to the archetype with the given ID, if it exists.
    #[inline]
    pub fn get_mut(&mut self, id: ArcheId) -> Option<&mut Archetype> {
        self.arches.get_mut(id.index())
    }

    /// Returns a reference to the archetype with the given ID without bounds checking.
    ///
    /// # Safety
    /// The caller must ensure that `id` is valid (within bounds of `arches`).
    /// Violating this condition leads to undefined behavior.
    #[inline]
    pub unsafe fn get_unchecked(&self, id: ArcheId) -> &Archetype {
        debug_assert!(id.index() < self.arches.len());
        unsafe { self.arches.get_unchecked(id.index()) }
    }

    /// Returns a mutable reference to the archetype with the given ID without bounds checking.
    ///
    /// # Safety
    /// The caller must ensure that `id` is valid (within bounds of `arches`).
    /// Violating this condition leads to undefined behavior.
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, id: ArcheId) -> &mut Archetype {
        debug_assert!(id.index() < self.arches.len());
        unsafe { self.arches.get_unchecked_mut(id.index()) }
    }

    /// Returns an iterator over the Archetypes.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, Archetype> {
        self.arches.iter()
    }

    /// Returns an iterator that allows modifying each Archetype.
    #[inline]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, Archetype> {
        self.arches.iter_mut()
    }
}
