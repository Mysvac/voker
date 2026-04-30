//! Component registration, collection, and writing system.
//!
//! This module provides the infrastructure for managing component lifecycle
//! operations, including registration of component types, collection of
//! required components, and writing component data to storage.

use core::any::TypeId;

use alloc::vec::Vec;
use voker_ptr::OwningPtr;
use voker_utils::hash::{SparseHashMap, SparseHashSet};

use crate::component::{Component, ComponentId, Components, StorageMode};
use crate::entity::Entity;
use crate::storage::{Maps, Table, TableId, TableRow};
use crate::tick::Tick;
use crate::utils::DebugCheckedUnwrap;
use crate::world::UnsafeWorld;

// -----------------------------------------------------------------------------
// ComponentRegistrar

/// A registrar for component types.
///
/// Provided for [`RequiredComponents`] implementation.
///
/// [`RequiredComponents`]: crate::component::RequiredComponents
#[repr(transparent)]
pub struct ComponentRegistrar<'a> {
    components: &'a mut Components,
}

impl<'a> ComponentRegistrar<'a> {
    /// Creates a new component registrar.
    #[inline]
    pub fn new(components: &'a mut Components) -> Self {
        Self { components }
    }

    /// Registers a component type with the system.
    ///
    /// This method ensures the component type is registered and assigned
    /// a unique ID for future operations.
    #[inline(never)] // we prohibit inlining to speed up compilation.
    pub fn register<T: Component>(&mut self) {
        self.components.register::<T>();
    }
}

// -----------------------------------------------------------------------------
// ComponentCollector

/// A collector for required components.
///
/// This struct handles the collection phase, where component dependencies
/// are recursively gathered and sorted by their storage type (dense or sparse).
pub struct ComponentCollector<'a> {
    components: &'a mut Components,
    dense: Vec<ComponentId>,
    sparse: Vec<ComponentId>,
    collected: SparseHashSet<ComponentId>,
}

/// Result of the component collection process.
///
/// Contains separate lists for dense and sparse components,
/// which can be either sorted or unsorted based on the collection method.
pub struct CollectResult {
    /// Component IDs for components using dense storage
    pub dense: Vec<ComponentId>,
    /// Component IDs for components using sparse storage
    pub sparse: Vec<ComponentId>,
}

impl<'a> ComponentCollector<'a> {
    /// Creates a new component collector.
    #[inline]
    pub fn new(components: &'a mut Components) -> Self {
        ComponentCollector {
            components,
            dense: Vec::new(),
            sparse: Vec::new(),
            collected: SparseHashSet::new(),
        }
    }

    /// Collects a component type, without required dependencies.
    ///
    /// This method registers the component if needed.
    #[inline(never)] // we prohibit inlining to speed up compilation.
    pub fn collect_explicit<T: Component>(&mut self) {
        let id = self.components.register::<T>();
        if self.collected.insert(id) {
            match T::STORAGE {
                StorageMode::Dense => {
                    self.dense.push(id);
                }
                StorageMode::Sparse => {
                    self.sparse.push(id);
                }
            }
        }
    }

    /// Collects a component type and its required dependencies.
    ///
    /// This method registers the component if needed, then recursively
    /// collects all components required by it.
    #[inline(never)] // we prohibit inlining to speed up compilation.
    pub fn collect_required<T: Component>(&mut self) {
        let id = self.components.register::<T>();
        if self.collected.insert(id) {
            match T::STORAGE {
                StorageMode::Dense => {
                    self.dense.push(id);
                }
                StorageMode::Sparse => {
                    self.sparse.push(id);
                }
            }
            if let Some(required) = T::REQUIRED {
                required.collect(self);
            }
        }
    }

    /// Returns the collected components with sorting applied.
    ///
    /// The component lists are sorted and deduplicated to ensure
    /// deterministic order and uniqueness.
    pub fn sorted(self) -> CollectResult {
        let mut dense = self.dense;
        let mut sparse = self.sparse;
        dense.sort_unstable();
        sparse.sort_unstable();
        dense.dedup();
        sparse.dedup();
        CollectResult { dense, sparse }
    }

    /// Returns the collected components without sorting.
    ///
    /// The `ComponentIds` may be duplicated in the result.
    ///
    /// This preserves the original collection order, which may be
    /// more efficient when order is not important.
    pub fn unsorted(self) -> CollectResult {
        CollectResult {
            dense: self.dense,
            sparse: self.sparse,
        }
    }
}

// -----------------------------------------------------------------------------
// ComponentWriter

/// Tracks whether a component was written as required or explicit.
///
/// This distinction is important for understanding component origin
/// and handling write conflicts appropriately.
enum WritedState {
    Required,
    Explicit,
}

/// A writer for component data to storage.
///
/// This struct manages the writing of component data to either dense tables
/// or sparse maps, handling both required and explicit writes.
///
/// # Writing Model
///
/// The writer is entity-scoped. Callers must provide the target [`Entity`],
/// along with the corresponding [`Table`] and [`TableRow`] context.
///
/// Internally, one `WritedState` is tracked per component ID to indicate
/// whether a component has already been written. This ensures repeated writes
/// replace prior values correctly and prevents memory leaks.
///
/// By default, components are treated as not yet written. If some component
/// slots are already initialized before using this writer, callers must mark
/// them with [`ComponentWriter::set_writed`] to avoid double initialization.
///
/// # Explicit vs Required Writes
///
/// Explicit writes have higher priority than automatically generated required
/// writes. For this reason, the API provides both [`write_required`] and
/// [`write_explicit`], with matching states in `WritedState`.
///
/// Intuitively, `Explicit` values may overwrite `Required` values, but not the
/// other way around. Entries marked by [`set_writed`] are always treated as `Explicit`.
///
/// [`write_required`]: ComponentWriter::write_required
/// [`write_explicit`]: ComponentWriter::write_explicit
/// [`set_writed`]: ComponentWriter::set_writed
///
/// # Safety
///
/// The writing operations require careful handling of memory safety:
/// - Components must be properly registered before writing
/// - Offsets must be valid within the data buffer
/// - Entity must be properly prepared to receive components
pub struct ComponentWriter<'a> {
    entity: Entity,
    tick: Tick,
    table_row: TableRow,
    table: &'a mut Table,
    maps: &'a mut Maps,
    components: &'a Components,
    writed: SparseHashMap<ComponentId, WritedState>,
}

impl ComponentWriter<'_> {
    /// # Safety
    /// Guaranteed by the caller.
    #[inline]
    pub unsafe fn new<'a>(
        world: UnsafeWorld<'a>,
        entity: Entity,
        table_id: TableId,
        table_row: TableRow,
    ) -> ComponentWriter<'a> {
        let world_mut = unsafe { world.data_mut() };
        // This writer only requires `data_mut` (not `full_mut`),
        // so it must use `this_run` instead of `this_run_fast`.
        let tick = world_mut.this_run_fast();
        let table = unsafe { world_mut.storages.tables.get_unchecked_mut(table_id) };
        let maps = &mut world_mut.storages.maps;
        let components = &world_mut.components;

        ComponentWriter {
            entity,
            tick,
            table_row,
            table,
            maps,
            components,
            writed: SparseHashMap::new(),
        }
    }

    /// Marks a component as already written explicitly.
    ///
    /// If [`ComponentWriter::write_required`] or
    /// [`ComponentWriter::write_explicit`] has already been called for this
    /// component, calling this method is unnecessary.
    ///
    /// This is typically used during insertion when some target component slots
    /// are already initialized and must be marked to prevent re-initialization.
    ///
    /// # Safety
    /// - `component` must be registered.
    /// - The corresponding component value must already be initialized.
    #[inline]
    pub unsafe fn set_writed(&mut self, component: ComponentId) {
        self.writed.insert(component, WritedState::Explicit);
    }

    /// Writes a required component using a constructor function.
    ///
    /// This method creates and writes a component that is required by
    /// the system, using the provided constructor.
    ///
    /// # Safety
    /// - `T` must be part of the target entity layout.
    /// - `T` must be registered and storage for it must be prepared.
    #[inline(never)]
    pub unsafe fn write_required<T: Component>(&mut self, func: impl FnOnce() -> T) {
        let type_id = TypeId::of::<T>();
        let component = unsafe { self.components.get_id(type_id).debug_checked_unwrap() };
        if !self.writed.contains_key(&component) {
            let data = func();
            voker_ptr::into_owning!(data);
            match T::STORAGE {
                StorageMode::Dense => unsafe {
                    self.init_dense(component, data);
                },
                StorageMode::Sparse => unsafe {
                    self.init_sparse(component, data);
                },
            }
        }
    }

    /// Writes an explicit component from a data buffer offset.
    ///
    /// This method writes a component that was explicitly provided,
    /// reading it from the internal data buffer at the specified offset.
    ///
    /// # Safety
    /// - `T` must be part of the target entity layout.
    /// - `T` must be registered and storage for it must be prepared.
    /// - `offset` must be valid in `self.data`.
    #[inline(never)]
    pub unsafe fn write_explicit<T: Component>(&mut self, data: OwningPtr<'_>) {
        let type_id = TypeId::of::<T>();
        let component = unsafe { self.components.get_id(type_id).debug_checked_unwrap() };

        match T::STORAGE {
            StorageMode::Dense => unsafe {
                self.write_dense(component, data);
            },
            StorageMode::Sparse => unsafe {
                self.write_sparse(component, data);
            },
        }
    }

    /// Writes an explicit component from a data buffer offset.
    #[inline(never)]
    pub unsafe fn write_custom<T: Component>(&mut self, data: T) {
        let type_id = TypeId::of::<T>();
        let component = unsafe { self.components.get_id(type_id).debug_checked_unwrap() };
        voker_ptr::into_owning!(data);
        match T::STORAGE {
            StorageMode::Dense => unsafe {
                self.write_dense(component, data);
            },
            StorageMode::Sparse => unsafe {
                self.write_sparse(component, data);
            },
        }
    }

    /// Initializes a new component in dense storage.
    ///
    /// # Safety
    /// Guaranteed by the caller.
    #[inline(never)]
    unsafe fn init_dense(&mut self, component: ComponentId, data: OwningPtr<'_>) {
        unsafe {
            let col = self.table.get_table_col(component).debug_checked_unwrap();
            self.table.init_item(col, self.table_row, data, self.tick);
            self.writed.insert(component, WritedState::Required);
        }
    }

    /// Initializes a new component in sparse storage.
    ///
    /// # Safety
    /// Guaranteed by the caller.
    #[inline(never)]
    unsafe fn init_sparse(&mut self, component: ComponentId, data: OwningPtr<'_>) {
        unsafe {
            let map_id = self.maps.get_id(component).debug_checked_unwrap();
            let map = self.maps.get_unchecked_mut(map_id);
            let row = map.get_map_row(self.entity).debug_checked_unwrap();
            map.init_item(row, data, self.tick);
            self.writed.insert(component, WritedState::Required);
        }
    }

    /// Writes or replaces a component in dense storage.
    ///
    /// # Safety
    /// Guaranteed by the caller.
    #[inline(never)]
    unsafe fn write_dense(&mut self, component: ComponentId, data: OwningPtr<'_>) {
        use voker_utils::hash::map::Entry;
        unsafe {
            let col = self.table.get_table_col(component).debug_checked_unwrap();
            let row = self.table_row;
            match self.writed.entry(component) {
                Entry::Occupied(mut entry) => {
                    self.table.replace_item(col, row, data, self.tick);
                    *entry.get_mut() = WritedState::Explicit;
                }
                Entry::Vacant(entry) => {
                    self.table.init_item(col, row, data, self.tick);
                    entry.insert(WritedState::Explicit);
                }
            }
        }
    }

    /// Writes or replaces a component in sparse storage.
    ///
    /// # Safety
    /// Guaranteed by the caller.
    #[inline(never)]
    unsafe fn write_sparse(&mut self, component: ComponentId, data: OwningPtr<'_>) {
        use voker_utils::hash::map::Entry;
        unsafe {
            let map_id = self.maps.get_id(component).debug_checked_unwrap();
            let map = self.maps.get_unchecked_mut(map_id);
            let row = map.get_map_row(self.entity).debug_checked_unwrap();
            match self.writed.entry(component) {
                Entry::Occupied(mut entry) => {
                    map.replace_item(row, data, self.tick);
                    *entry.get_mut() = WritedState::Explicit;
                }
                Entry::Vacant(entry) => {
                    map.init_item(row, data, self.tick);
                    entry.insert(WritedState::Explicit);
                }
            }
        }
    }
}
