use core::any::TypeId;
use core::fmt::Debug;

use alloc::vec::Vec;

use voker_os::sync::Arc;
use voker_utils::extra::TypeIdMap;
use voker_utils::hash::HashMap;

use crate::bundle::BundleId;
use crate::component::ComponentId;

// -----------------------------------------------------------------------------
// BundleInfo

/// Metadata information about a registered component bundle.
///
/// A bundle is a collection of components that are typically inserted or
/// removed together. This struct stores the component composition of a bundle,
/// including which components are stored densely (in tables) versus sparsely
/// (in maps).
pub struct BundleInfo {
    // A unique identifier for a Bundle.
    // Also the index in the archetypes array
    id: BundleId,
    // Use u32 to reduce the size of the struct.
    dense_len: u32,
    // - `[..dense_len]` are stored in Tables, sorted.
    // - `[dense_len..]` sare stored in Maps, sorted.
    // We use Arc to reduce memory allocation overhead.
    components: Arc<[ComponentId]>,
}

impl BundleInfo {
    #[inline(always)]
    pub(crate) fn dense_len(&self) -> usize {
        self.dense_len as usize
    }

    #[inline(always)]
    pub(crate) fn clone_components(&self) -> Arc<[ComponentId]> {
        self.components.clone()
    }
}

impl BundleInfo {
    /// Returns the unique identifier of this bundle.
    #[inline(always)]
    pub fn id(&self) -> BundleId {
        self.id
    }

    /// Returns the complete list of component types in this bundle.
    #[inline(always)]
    pub fn components(&self) -> &[ComponentId] {
        &self.components
    }

    /// Returns the list of dense component types in this bundle.
    #[inline(always)]
    pub fn dense_components(&self) -> &[ComponentId] {
        &self.components[..self.dense_len as usize]
    }

    /// Returns the list of sparse component types in this bundle.
    #[inline(always)]
    pub fn sparse_components(&self) -> &[ComponentId] {
        &self.components[self.dense_len as usize..]
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
}

impl Debug for BundleInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bundle")
            .field("id", &self.id)
            .field("components", &self.components)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Bundles

/// A registry for managing all component bundles in the ECS world.
///
/// This structure maintains mappings between bundle types and their metadata,
/// providing efficient lookup by both type ID and component set. It ensures
/// that identical component sets are assigned the same bundle ID, preventing
/// duplication and enabling bundle sharing.
pub struct Bundles {
    infos: Vec<BundleInfo>,
    mapper: HashMap<Arc<[ComponentId]>, BundleId>,
    type_mapper: TypeIdMap<BundleId>,
}

impl Debug for Bundles {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.infos, f)
    }
}

impl Bundles {
    /// Creates a new bundle registry, initializes with
    /// the empty bundle (no components).
    pub(crate) fn new() -> Self {
        let components: Arc<[ComponentId]> = Arc::new([]);
        let mut val = const {
            Bundles {
                infos: Vec::new(),
                mapper: HashMap::new(),
                type_mapper: TypeIdMap::new(),
            }
        };

        val.mapper.insert(components.clone(), BundleId::EMPTY);
        val.type_mapper.insert(TypeId::of::<()>(), BundleId::EMPTY);
        val.infos.push(BundleInfo {
            id: BundleId::EMPTY,
            dense_len: 0,
            components,
        });

        val
    }

    /// Registers a new bundle type or returns an existing bundle ID.
    ///
    /// If a bundle with the exact same component set already exists, returns
    /// its ID and also maps the new type ID to it. Otherwise, creates a new
    /// bundle entry with a fresh ID.
    ///
    /// # Safety
    /// - Component IDs must be valid and properly registered, not duplicated.
    /// - The components in `0..dense_len` must be sorted and storage in dense.
    /// - The components in `dense_len..` must be sparse, and storage in sparse.
    pub(crate) unsafe fn register(
        &mut self,
        type_id: TypeId,
        components: &[ComponentId],
        dense_len: u32,
    ) -> BundleId {
        if let Some(&id) = self.mapper.get(components) {
            self.type_mapper.insert(type_id, id);
            id
        } else {
            let index = self.infos.len();
            assert!(index < u32::MAX as usize, "too many bundles");
            let id = BundleId::new(index as u32);

            let arc: Arc<[ComponentId]> = components.into();

            self.infos.push(BundleInfo {
                id,
                dense_len,
                components: arc.clone(),
            });
            self.mapper.insert(arc, id);
            self.type_mapper.insert(type_id, id);

            id
        }
    }
}

impl Bundles {
    /// Returns the number of registered bundles.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "len > 0")]
    pub const fn len(&self) -> usize {
        self.infos.len()
    }

    /// Returns the bundle information for a given bundle ID, if it exists.
    #[inline]
    pub fn get(&self, id: BundleId) -> Option<&BundleInfo> {
        self.infos.get(id.index())
    }

    /// Returns the bundle information for a given bundle ID without bounds checking.
    ///
    /// # Safety
    /// The caller must ensure the bundle ID is valid (within bounds).
    #[inline]
    pub unsafe fn get_unchecked(&self, id: BundleId) -> &BundleInfo {
        debug_assert!(id.index() < self.infos.len());
        unsafe { self.infos.get_unchecked(id.index()) }
    }

    /// Returns the bundle ID associated with ComponentIds, if it exists.
    pub fn get_id(&self, components: &[ComponentId]) -> Option<BundleId> {
        self.mapper.get(components).copied()
    }

    /// Returns the bundle ID associated with a type ID, if it exists.
    pub fn get_id_by_type(&self, id: TypeId) -> Option<BundleId> {
        self.type_mapper.get(id).copied()
    }

    /// Returns an iterator over the BundleInfo.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, BundleInfo> {
        self.infos.iter()
    }
}
