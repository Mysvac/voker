use core::fmt::Debug;

use voker_os::sync::Arc;

use super::BundleId;
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
    pub(super) fn new(
        bundle_id: BundleId,
        dense_len: usize,
        components: Arc<[ComponentId]>,
    ) -> Self {
        debug_assert!(components[..dense_len].is_sorted());
        debug_assert!(components[dense_len..].is_sorted());
        Self {
            id: bundle_id,
            // SAFETY: ComponentId < u32::MAX
            dense_len: dense_len as u32,
            components,
        }
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
