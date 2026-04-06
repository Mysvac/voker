use core::fmt::Debug;

use super::BundleId;
use crate::component::ComponentId;

// -----------------------------------------------------------------------------
// BundleInfo

/// Metadata information about a registered component bundle.
///
/// `BundleInfo` is similar to `Archetype`, but maintains less
/// information (only the component list, no entity data).
///
/// Note that a single `BundleInfo` can be shared by multiple different
/// `Bundle` types as long as they have the exact same component set.
///
/// Conversely, a concrete bundle type may have **two** associated `BundleInfo`s:
/// the explicit component set and the complete (required) component set.
pub struct BundleInfo {
    id: BundleId,
    // Use u32 to reduce the size of the struct.
    dense_len: u32,
    // - `[..dense_len]` are stored in Tables, sorted.
    // - `[dense_len..]` are stored in Maps, sorted.
    components: &'static [ComponentId],
}

// -----------------------------------------------------------------------------
// Private

impl BundleInfo {
    /// Create a `BundleInfo` from given information.
    pub(super) fn new(
        bundle_id: BundleId,
        dense_len: usize,
        components: &'static [ComponentId],
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
}

// -----------------------------------------------------------------------------
// Basic

impl Debug for BundleInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bundle")
            .field("id", &self.id)
            .field("components", &self.components)
            .finish()
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
    pub fn components(&self) -> &'static [ComponentId] {
        &self.components
    }

    /// Returns the list of dense component types in this bundle.
    #[inline(always)]
    pub fn dense_components(&self) -> &'static [ComponentId] {
        &self.components[..self.dense_len as usize]
    }

    /// Returns the list of sparse component types in this bundle.
    #[inline(always)]
    pub fn sparse_components(&self) -> &'static [ComponentId] {
        &self.components[self.dense_len as usize..]
    }

    /// Checks if this archetype contains a specific component type.
    ///
    /// # Complexity
    /// - Time: O(log n) where n is the total number of component types
    pub fn contains_component(&self, id: ComponentId) -> bool {
        self.contains_dense_component(id) || self.contains_sparse_component(id)
    }

    /// Checks if this archetype contains a specific dense component type.
    ///
    /// # Complexity
    /// - Time: O(log n) where n is the number of dense components
    pub fn contains_dense_component(&self, id: ComponentId) -> bool {
        self.dense_components().binary_search(&id).is_ok()
    }

    /// Checks if this archetype contains a specific sparse component type.
    ///
    /// # Complexity
    /// - Time: O(log n) where n is the number of sparse components
    pub fn contains_sparse_component(&self, id: ComponentId) -> bool {
        self.sparse_components().binary_search(&id).is_ok()
    }
}
