use core::any::TypeId;
use core::fmt::Debug;

use alloc::vec::Vec;

use voker_utils::extra::TypeIdMap;
use voker_utils::hash::HashMap;

use super::{BundleId, BundleInfo};
use crate::component::ComponentId;

// -----------------------------------------------------------------------------
// Bundles

/// A collection of [`BundleInfo`]s.
pub struct Bundles {
    infos: Vec<BundleInfo>,
    mapper: HashMap<&'static [ComponentId], BundleId>,
    explicit: TypeIdMap<BundleId>,
    required: TypeIdMap<BundleId>,
}

impl Debug for Bundles {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(self.infos.as_slice(), f)
    }
}

impl Bundles {
    /// Creates a new `Bundles`, initialized with the *Empty `BundleInfo`*.
    pub(crate) fn new() -> Self {
        let mut val = Bundles {
            infos: Vec::new(),
            mapper: HashMap::new(),
            explicit: TypeIdMap::new(),
            required: TypeIdMap::new(),
        };

        val.infos.push(BundleInfo::new(BundleId::EMPTY, 0, &[]));
        val.mapper.insert(&[], BundleId::EMPTY);
        val.explicit.insert(TypeId::of::<()>(), BundleId::EMPTY);
        val.required.insert(TypeId::of::<()>(), BundleId::EMPTY);

        val
    }

    /// Registers a new bundle for explicit components and returns its ID.
    ///
    /// If the target bundle already exists, return it directly.
    ///
    /// # Safety
    /// - Component IDs must be valid and properly registered, not duplicated.
    /// - The components in `..dense_len` must be sorted and storage in dense.
    /// - The components in `dense_len..` must be sorted and storage in sparse.
    pub(crate) unsafe fn register_explicit(
        &mut self,
        type_id: TypeId,
        components: &'static [ComponentId],
        dense_len: usize,
    ) -> BundleId {
        if let Some(&id) = self.mapper.get(components) {
            self.explicit.insert(type_id, id);
            id
        } else {
            core::hint::cold_path();
            let index = self.infos.len();
            let id = BundleId::new(index as u32);

            self.infos.push(BundleInfo::new(id, dense_len, components));
            self.mapper.insert(components, id);
            self.explicit.insert(type_id, id);

            id
        }
    }

    /// Registers a new bundle for required components and returns its ID.
    ///
    /// If the target bundle already exists, return it directly.
    ///
    /// # Safety
    /// - Component IDs must be valid and properly registered, not duplicated.
    /// - The components in `..dense_len` must be sorted and storage in dense.
    /// - The components in `dense_len..` must be sorted and storage in sparse.
    pub(crate) unsafe fn register_required(
        &mut self,
        type_id: TypeId,
        components: &'static [ComponentId],
        dense_len: usize,
    ) -> BundleId {
        if let Some(&id) = self.mapper.get(components) {
            self.required.insert(type_id, id);
            id
        } else {
            core::hint::cold_path();
            let index = self.infos.len();
            let id = BundleId::new(index as u32);

            self.infos.push(BundleInfo::new(id, dense_len, components));
            self.mapper.insert(components, id);
            self.required.insert(type_id, id);

            id
        }
    }

    /// Registers a new bundle from given component ids.
    ///
    /// The target bundle **must be unregistered**. In other world,
    /// `self.get_id()` must return `None` before this function call.
    ///
    /// # Safety
    /// - Component IDs must be valid and properly registered, not duplicated.
    /// - The components in `..dense_len` must be sorted and storage in dense.
    /// - The components in `dense_len..` must be sorted and storage in sparse.
    pub(crate) unsafe fn register_dynamic_unique(
        &mut self,
        components: &'static [ComponentId],
        dense_len: usize,
    ) -> BundleId {
        debug_assert!(!self.mapper.contains_key(components));

        let index = self.infos.len();
        let id = BundleId::new(index as u32);

        self.infos.push(BundleInfo::new(id, dense_len, components));
        self.mapper.insert(components, id);

        id
    }
}

impl Bundles {
    /// Returns the number of registered bundles.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "len > 0")]
    pub fn len(&self) -> usize {
        self.infos.len()
    }

    /// Returns the bundle ID associated with `ComponentIds`, if it exists.
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
        self.required.get(id).copied()
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

    /// Extracts a slice containing the entire Bundles.
    #[inline]
    pub fn as_slice(&self) -> &[BundleInfo] {
        self.infos.as_slice()
    }

    /// Returns an iterator over the `BundleInfo` values.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, BundleInfo> {
        self.infos.iter()
    }
}
