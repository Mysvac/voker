use voker_task::ComputeTaskPool;

use crate::component::{ComponentInfo, ComponentStorage};
use crate::resource::ResourceInfo;
use crate::storage::{Maps, Tables};
use crate::tick::CheckTicks;

use super::ResSet;

// -----------------------------------------------------------------------------
// Storages

/// Central coordinator for all ECS storage backends.
///
/// `Storages` aggregates and manages the three primary storage systems in the ECS:
/// - **Resources** (`ResSet`) - Singleton data attached to the world itself
/// - **Tables** (`Tables`) - Dense, contiguous storage for components (archetype-based)
/// - **Maps** (`Maps`) - Sparse storage for components that don't benefit from dense storage
///
/// This structure serves as the single point of access for all component and resource
/// data, routing operations to the appropriate storage backend based on component
/// type and storage strategy.
///
/// # Storage Strategies
///
/// Components are assigned to either dense or sparse storage based on their
/// [`ComponentStorage`] setting:
/// - **Dense** (`ComponentStorage::Dense`): Stored in [`Tables`] - optimized for
///   components present on many entities
/// - **Sparse** (`ComponentStorage::Sparse`): Stored in [`Maps`] - optimized for
///   rarely-present components or large component sets
#[derive(Debug)]
pub struct Storages {
    pub ress: ResSet,
    pub tables: Tables,
    pub maps: Maps,
}

impl Storages {
    /// Creates a new, empty storage coordinator.
    pub(crate) fn new() -> Storages {
        Storages {
            ress: ResSet::new(),
            tables: Tables::new(),
            maps: Maps::new(),
        }
    }

    /// Prepares storage for a resource type.
    ///
    /// # Resource Lifecycle States
    ///
    /// Resources in the ECS progress through several states during their lifetime:
    ///
    /// |    State         | Description                            | Storage Status                       |
    /// |------------------|----------------------------------------|--------------------------------------|
    /// | **Unregistered** | No `ResourceId` allocated yet          | Not tracked                          |
    /// | **Unprepared**   | `Id` allocated, but no memory reserved | `ResData` not initialized            |
    /// | **Prepared**     | Memory allocated, ready for insertion  | Storage reserved, data uninitialized |
    /// | **Inserted**     | Memory allocated and initialized       | Active resource                      |
    ///
    /// This method transitions a resource from **unprepared** to **prepared** state.
    /// First call may allocate, subsequent calls are no-op
    #[inline]
    pub fn prepare_resource(&mut self, info: &ResourceInfo) {
        self.ress.prepare(info);
    }

    /// Prepares storage for a component type based on its storage strategy.
    ///
    /// # Component Registration vs Preparation
    ///
    /// Component management involves two distinct phases:
    ///
    /// |     Phase    |                    Purpose                       |
    /// |--------------|--------------------------------------------------|
    /// | **Register** | Assign unique `ComponentId`, store metadata      |
    /// | **Prepare**  | Reserve storage space in the appropriate backend |
    ///
    /// The behavior of this method differs based on the component's storage strategy:
    ///
    /// ## Dense Components ([`ComponentStorage::Dense`])
    /// * **No immediate allocation** - Memory is allocated lazily when tables are created
    /// * Preparation is a no-op - Tables handle allocation on demand
    /// * Best for components present on many entities
    ///
    /// ## Sparse Components ([`ComponentStorage::Sparse`])
    /// * **Immediate allocation** - Creates a dedicated [`crate::storage::Map`] instance
    /// * Each sparse component gets its own map for O(1) lookup
    /// * Best for rarely-present components or large component sets
    #[inline]
    pub fn prepare_component(&mut self, info: &ComponentInfo) {
        match info.storage() {
            ComponentStorage::Dense => {
                self.tables.prepare(info);
            }
            ComponentStorage::Sparse => {
                self.maps.prepare(info);
            }
        }
    }

    /// Updates tick information across all storage backends.
    ///
    /// This method advances the tick counters for all stored data, marking which
    /// components and resources have been accessed or modified. It automatically
    /// parallelizes the work when a [`ComputeTaskPool`] is available.
    ///
    /// # Parallelism
    /// When a compute task pool is available, this method spawns separate tasks for:
    /// - Resource set tick updates
    /// - Each individual table's tick updates  
    /// - Each individual map's tick updates
    ///
    /// This provides near-optimal parallel utilization for large worlds with
    /// many tables and maps.
    pub fn check_ticks(&mut self, check: CheckTicks) {
        let Storages { ress, tables, maps } = self;

        if let Some(task_pool) = ComputeTaskPool::try_get() {
            task_pool.scope(|scope| {
                scope.spawn(async move {
                    ress.check_ticks(check);
                });
                tables.tables.iter_mut().for_each(|tb| {
                    scope.spawn(async move { tb.check_ticks(check) });
                });
                maps.maps.iter_mut().for_each(|mp| {
                    scope.spawn(async move { mp.check_ticks(check) });
                });
            });
        } else {
            ress.check_ticks(check);
            tables.tables.iter_mut().for_each(|tb| {
                tb.check_ticks(check);
            });
            maps.maps.iter_mut().for_each(|mp| {
                mp.check_ticks(check);
            });
        }
    }
}
