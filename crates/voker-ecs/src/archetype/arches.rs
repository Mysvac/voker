use alloc::vec::Vec;
use core::fmt::Debug;

use voker_utils::hash::HashMap;

use crate::archetype::{ArcheId, Archetype};
use crate::bundle::BundleId;
use crate::component::{ComponentId, Components};
use crate::storage::TableId;

// -----------------------------------------------------------------------------
// Archetypes

/// A collection of [`Archetype`]s.
pub struct Archetypes {
    arches: Vec<Archetype>,
    mapper: HashMap<&'static [ComponentId], ArcheId>,
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
    /// Creates a new `Archetypes`, initializes with the *Empty Archetype*.
    pub(crate) fn new() -> Self {
        let mut val = Archetypes {
            arches: Vec::new(),
            bundles: Vec::new(),
            mapper: HashMap::new(),
        };

        let arche = Archetype::new(ArcheId::EMPTY, TableId::EMPTY, 0, &[], &Components::new());

        val.arches.push(arche);
        val.mapper.insert(&[], ArcheId::EMPTY);
        val.bundles.push(Some(ArcheId::EMPTY));

        val
    }

    /// Map a bundle_id to arche_id, then you can get it through `get_id_by_bundle`.
    pub(crate) fn map_bundle_id(&mut self, bundle_id: BundleId, arche_id: ArcheId) {
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

    /// Register a new Archetype from given information and return it's ID.
    ///
    /// # Safety
    /// - The archetype is **unique**(unregistered).
    /// - All `ComponentId`s in `idents` are registered.
    /// - `idents[..dense_len]` and `[dense_len..]` are both sorted and deduplicated.
    /// - `table_id` and `dense_len` are valid for the given component layout.
    pub(crate) unsafe fn register_unique(
        &mut self,
        table_id: TableId,
        dense_len: usize,
        idents: &'static [ComponentId],
        components: &Components,
    ) -> ArcheId {
        debug_assert!(idents[..dense_len].is_sorted());
        debug_assert!(idents[dense_len..].is_sorted());

        // Panic if len == u32::MAX, so the id will not wrap.
        let arche_id = ArcheId::new(self.arches.len() as u32);
        let arche = Archetype::new(arche_id, table_id, dense_len, idents, components);

        self.arches.push(arche);
        self.mapper.insert(idents, arche_id);

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
    /// `ArcheId` is valid (within bounds of `arches`).
    #[inline]
    pub unsafe fn get_unchecked(&self, id: ArcheId) -> &Archetype {
        debug_assert!(id.index() < self.arches.len());
        unsafe { self.arches.get_unchecked(id.index()) }
    }

    /// Returns a mutable reference to the archetype with the given ID without bounds checking.
    ///
    /// # Safety
    /// `ArcheId` is valid (within bounds of `arches`).
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, id: ArcheId) -> &mut Archetype {
        debug_assert!(id.index() < self.arches.len());
        unsafe { self.arches.get_unchecked_mut(id.index()) }
    }

    /// Extracts a slice containing the entire Archetypes.
    #[inline]
    pub fn as_slice(&self) -> &[Archetype] {
        self.arches.as_slice()
    }

    /// Extracts a mutable slice of the entire Archetypes.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Archetype] {
        self.arches.as_mut_slice()
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
