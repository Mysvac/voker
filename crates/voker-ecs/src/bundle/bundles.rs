use core::any::TypeId;
use core::fmt::Debug;

use alloc::vec::Vec;

use voker_os::sync::Arc;
use voker_utils::extra::TypeIdMap;
use voker_utils::hash::HashMap;

use super::{BundleId, BundleInfo};
use crate::component::ComponentId;

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
    explicit: TypeIdMap<BundleId>,
    required: TypeIdMap<BundleId>,
}

impl Debug for Bundles {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(self.infos.as_slice(), f)
    }
}

impl Bundles {
    /// Creates a new bundle registry, initializes with
    /// the empty bundle (no components).
    pub(crate) fn new() -> Self {
        let mut val = Bundles {
            infos: Vec::new(),
            mapper: HashMap::new(),
            explicit: TypeIdMap::new(),
            required: TypeIdMap::new(),
        };

        val.infos.push(BundleInfo::new(BundleId::EMPTY, 0, Arc::new([])));
        val.mapper.insert(Arc::new([]), BundleId::EMPTY);
        val.explicit.insert(TypeId::of::<()>(), BundleId::EMPTY);
        val.required.insert(TypeId::of::<()>(), BundleId::EMPTY);

        val
    }

    /// Registers a new bundle for explicit components and returns its ID.
    ///
    /// If the target bundle already exist, return it directly.
    ///
    /// # Safety
    /// - Component IDs must be valid and properly registered, not duplicated.
    /// - The components in `..dense_len` must be sorted and storage in dense.
    /// - The components in `dense_len..` must be sorted and storage in sparse.
    pub(crate) unsafe fn register_explicit(
        &mut self,
        type_id: TypeId,
        components: &[ComponentId],
        dense_len: usize,
    ) -> BundleId {
        if let Some(&id) = self.mapper.get(components) {
            self.explicit.insert(type_id, id);
            id
        } else {
            let index = self.infos.len();
            assert!(index < u32::MAX as usize, "too many bundles");
            let id = BundleId::new(index as u32);

            let arc: Arc<[ComponentId]> = components.into();

            self.infos.push(BundleInfo::new(id, dense_len, arc.clone()));
            self.mapper.insert(arc, id);
            self.explicit.insert(type_id, id);

            id
        }
    }

    /// Registers a new bundle for required components and returns its ID.
    ///
    /// This `required` including `explicit`.
    ///
    /// If the target bundle already exist, return it directly.
    ///
    /// # Safety
    /// - Component IDs must be valid and properly registered, not duplicated.
    /// - The components in `..dense_len` must be sorted and storage in dense.
    /// - The components in `dense_len..` must be sorted and storage in sparse.
    pub(crate) unsafe fn register_required(
        &mut self,
        type_id: TypeId,
        components: &[ComponentId],
        dense_len: usize,
    ) -> BundleId {
        if let Some(&id) = self.mapper.get(components) {
            self.required.insert(type_id, id);
            id
        } else {
            let index = self.infos.len();
            assert!(index < u32::MAX as usize, "too many bundles");
            let id = BundleId::new(index as u32);

            let arc: Arc<[ComponentId]> = components.into();

            self.infos.push(BundleInfo::new(id, dense_len, arc.clone()));
            self.mapper.insert(arc, id);
            self.required.insert(type_id, id);

            id
        }
    }
}

impl Bundles {
    /// Returns the number of registered bundles.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "len > 0")]
    pub fn len(&self) -> usize {
        self.infos.len()
    }

    /// Returns the bundle ID associated with ComponentIds, if it exists.
    #[inline]
    pub fn get_id(&self, components: &[ComponentId]) -> Option<BundleId> {
        self.mapper.get(components).copied()
    }

    /// Returns the explicit bundle ID associated with a type ID, if it exists.
    #[inline]
    pub fn get_explicit_id(&self, id: TypeId) -> Option<BundleId> {
        self.explicit.get(id).copied()
    }

    /// Returns the required bundle ID associated with a type ID, if it exists.
    #[inline]
    pub fn get_required_id(&self, id: TypeId) -> Option<BundleId> {
        self.explicit.get(id).copied()
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
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, id: BundleId) -> &BundleInfo {
        debug_assert!(id.index() < self.infos.len());
        unsafe { self.infos.get_unchecked(id.index()) }
    }

    /// Returns an iterator over the BundleInfo.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, BundleInfo> {
        self.infos.iter()
    }
}
