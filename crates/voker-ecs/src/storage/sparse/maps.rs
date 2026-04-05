use alloc::vec::Vec;
use core::fmt::Debug;

use voker_utils::hash::SparseHashMap;

use crate::component::{ComponentId, ComponentInfo, Components};
use crate::storage::{Map, MapId};

// -----------------------------------------------------------------------------
// Maps

/// A collection of sparse component maps.
///
/// |    Component A     |    Component B     |    Component C    | .. |
/// |--------------------|--------------------|-------------------|----|
/// | Map<Entity, Data>  | Map<Entity, Data>  | Map<Entity, Data> | .. |
///
/// `Maps` manages all [`Map`] instances for components that use sparse storage.
/// Each component type with sparse storage gets its own dedicated map that
/// maintains the mapping from entities to their component data.
///
/// # Storage Strategy
/// Sparse storage is ideal for components that:
/// - Are present on relatively few entities
/// - Are frequently added and removed
/// - Benefit from entity-to-component lookup performance
pub struct Maps {
    maps: Vec<Map>,
    mapper: SparseHashMap<ComponentId, MapId>,
}

// -----------------------------------------------------------------------------
// Private

impl Maps {
    /// Creates a new empty `Maps` collection.
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            maps: Vec::new(),
            mapper: SparseHashMap::new(),
        }
    }

    /// Prepares a new map for a component type if it doesn't already exist.
    ///
    /// This function ensures that a sparse map is created for components
    /// marked with sparse storage.
    pub(crate) fn prepare(&mut self, info: &ComponentInfo) {
        debug_assert!(info.storage().is_sparse());

        if !self.mapper.contains_key(&info.id()) {
            let map_id = MapId::new(self.maps.len() as u32);
            let map = Map::new(map_id, info);
            self.maps.push(map);
            self.mapper.insert(info.id(), map_id);
        }
    }

    /// Registers multiple components for sparse storage.
    ///
    /// This is typically called during archetype registrationn.
    ///
    /// # Safety
    /// - All `ComponentId`s in `idents` must be valid and registered in `components`
    /// - The caller must ensure proper synchronization
    pub(crate) unsafe fn register(&mut self, components: &Components, idents: &[ComponentId]) {
        idents.iter().for_each(|&id| {
            let info = unsafe { components.get_unchecked(id) };
            self.prepare(info);
        });
    }
}

// -----------------------------------------------------------------------------
// Basic

impl Debug for Maps {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(self.maps.as_slice(), f)
    }
}

impl Maps {
    /// Returns the number of Maps.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "consistency")]
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Returns the ID of the map for the given component, if it exists.
    #[inline]
    pub fn get_id(&self, component: ComponentId) -> Option<MapId> {
        self.mapper.get(&component).copied()
    }

    /// Gets a reference to the map with the given ID.
    #[inline]
    pub fn get(&self, id: MapId) -> Option<&Map> {
        self.maps.get(id.index())
    }

    /// Gets a reference to the map with the given ID without bounds checking.
    #[inline]
    pub fn get_mut(&mut self, id: MapId) -> Option<&mut Map> {
        self.maps.get_mut(id.index())
    }

    /// Gets a reference to the map with the given ID without bounds checking.
    ///
    /// # Safety
    /// - `id.index()` must be within bounds of `self.maps`
    /// - The caller must ensure the map exists at this ID
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, id: MapId) -> &Map {
        debug_assert!(id.index() < self.maps.len());
        unsafe { self.maps.get_unchecked(id.index()) }
    }

    /// Gets a mutable reference to the map with the given ID without bounds checking.
    ///
    /// # Safety
    /// - `id.index()` must be within bounds of `self.maps`
    /// - The caller must ensure the map exists at this ID
    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, id: MapId) -> &mut Map {
        debug_assert!(id.index() < self.maps.len());
        unsafe { self.maps.get_unchecked_mut(id.index()) }
    }

    /// Returns an iterator over the tables.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, Map> {
        self.maps.iter()
    }

    /// Returns an iterator that allows modifying each table.
    #[inline]
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, Map> {
        self.maps.iter_mut()
    }
}
