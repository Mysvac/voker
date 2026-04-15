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
        self.components
    }

    /// Returns the list of dense component types in this bundle.
    #[inline(always)]
    pub fn dense_components(&self) -> &'static [ComponentId] {
        let len = self.dense_len as usize;

        #[cfg(not(debug_assertions))]
        unsafe {
            // Disable boundary checking during release.
            ::core::hint::assert_unchecked(len <= self.components.len());
        }

        &self.components[..len]
    }

    /// Returns the list of sparse component types in this bundle.
    #[inline(always)]
    pub fn sparse_components(&self) -> &'static [ComponentId] {
        let len = self.dense_len as usize;

        #[cfg(not(debug_assertions))]
        unsafe {
            // Disable boundary checking during release.
            ::core::hint::assert_unchecked(len <= self.components.len());
        }

        &self.components[len..]
    }

    /// Checks if this archetype contains a specific component type.
    ///
    /// The number of components in a Bundle is usually small, making it
    /// very fast to determine under SIMD optimization.
    pub fn contains_component(&self, id: ComponentId) -> bool {
        crate::utils::contains_component(id, self.components)
    }

    /// Checks if this archetype contains a specific dense component type.
    ///
    /// The number of components in a Bundle is usually small, making it
    /// very fast to determine under SIMD optimization.
    pub fn contains_dense_component(&self, id: ComponentId) -> bool {
        crate::utils::contains_component(id, self.dense_components())
    }

    /// Checks if this archetype contains a specific sparse component type.
    ///
    /// The number of components in a Bundle is usually small, making it
    /// very fast to determine under SIMD optimization.
    pub fn contains_sparse_component(&self, id: ComponentId) -> bool {
        crate::utils::contains_component(id, self.sparse_components())
    }
}
