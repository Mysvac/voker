use alloc::vec::Vec;
use core::fmt::Debug;
use core::iter::FusedIterator;

use crate::archetype::{ArcheId, ArcheRow};
use crate::entity::error::{DespawnError, FetchError, MoveError, SpawnError};
use crate::entity::{Entity, EntityId, EntityTag};
use crate::storage::{TableId, TableRow};

// -----------------------------------------------------------------------------
// EntityLocation

/// Represents the precise storage location of an entity within the ECS world.
///
/// An `EntityLocation` contains both archetype and table coordinates, allowing
/// direct access to the entity's component data. This is used internally by
/// the ECS to track and retrieve entities efficiently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityLocation {
    pub arche_id: ArcheId,
    pub table_id: TableId,
    pub arche_row: ArcheRow,
    pub table_row: TableRow,
}

// -----------------------------------------------------------------------------
// EntityInfo

/// Internal tracking information for a single entity.
#[derive(Debug, Clone, Copy)]
struct EntityInfo {
    tag: EntityTag,
    location: Option<EntityLocation>,
}

const DEFAULT_INFO: EntityInfo = EntityInfo {
    tag: EntityTag::FIRST,
    location: None,
};

// -----------------------------------------------------------------------------
// Entities

/// Central registry for all entity metadata in the ECS world.
///
/// `Entities` maintains a sparse set of all entity IDs that have ever been
/// allocated, tracking their current tag and storage location. It
/// provides methods for spawning, despawning, and locating entities while
/// ensuring type safety through tag counters.
///
/// # Generations
///
/// Each entity slot has a tag counter that increments when the entity
/// is despawned and the slot becomes available for reuse. This prevents the
/// "stale reference" problem where a component reference could accidentally
/// access data belonging to a different entity that now occupies the same slot.
///
/// # Storage
///
/// The registry uses a dense vector indexed by entity ID, with holes for
/// unused slots. This provides O(1) lookup while maintaining reasonable
/// memory usage.
pub struct Entities {
    infos: Vec<EntityInfo>,
}

impl Debug for Entities {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        struct FormatLocation(Option<EntityLocation>);

        impl Debug for FormatLocation {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self.0 {
                    Some(l) => Debug::fmt(&l, f),
                    None => f.pad("None"),
                }
            }
        }

        fn entity_from(id: usize, tag: EntityTag) -> Entity {
            Entity::new(EntityId::without_provenance(id), tag)
        }

        let iter = self
            .infos
            .iter()
            .enumerate()
            .skip(1) // Skip invalid EntityId
            .map(|(id, info)| (entity_from(id, info.tag), info.location))
            .map(|(e, l)| (e, FormatLocation(l)));

        f.debug_list().entries(iter).finish()
    }
}

impl Entities {
    /// Creates a new empty entity registry.
    pub(crate) const fn new() -> Self {
        Self { infos: Vec::new() }
    }

    /// Return the number of **spawned** entities.
    ///
    /// # Complexity
    /// time: O(N)
    pub fn count_spawned(&self) -> usize {
        self.infos.iter().filter(|info| info.location.is_some()).count()
    }

    /// Resolves an entity ID to its current entity with correct tag.
    ///
    /// # Complexity
    /// time: O(1)
    pub fn resolve(&self, id: EntityId) -> Entity {
        let info = self.infos.get(id.index()).unwrap_or(&DEFAULT_INFO);

        Entity::new(id, info.tag)
    }

    /// Tries to retrieve the location of a spawned entity.
    ///
    /// # Returns
    /// - `Ok(Some(EntityLocation))` - The entity's current storage location
    /// - `Ok(None)` - The entity is not spawned but the tag matches.
    /// - `Err(FetchError)` - Tag mismatches.
    ///
    /// # Errors
    /// - `FetchError::Mismatch` - Generation counter mismatch (stale entity)
    pub fn try_locate(&self, entity: Entity) -> Result<Option<EntityLocation>, FetchError> {
        let info = self.infos.get(entity.index()).unwrap_or(&DEFAULT_INFO);

        if info.tag != entity.tag() {
            core::hint::cold_path();
            let expect = entity;
            let actual = Entity::new(entity.id(), info.tag);
            return Err(FetchError::Mismatch { expect, actual });
        }

        Ok(info.location)
    }

    /// Retrieves the location of a spawned entity.
    ///
    /// # Returns
    /// - `Ok(EntityLocation)` - The entity's current storage location
    /// - `Err(FetchError)` - If the entity doesn't exist, tag mismatches,
    ///   or the entity is not spawned
    ///
    /// # Errors
    /// - `FetchError::NotFound` - Entity index out of bounds
    /// - `FetchError::Mismatch` - Generation counter mismatch (stale entity)
    /// - `FetchError::NotSpawned` - Entity exists but is not spawned
    pub fn locate(&self, entity: Entity) -> Result<EntityLocation, FetchError> {
        let Some(info) = self.infos.get(entity.index()) else {
            core::hint::cold_path();
            return Err(FetchError::NotFound(entity.id()));
        };
        if info.tag != entity.tag() {
            core::hint::cold_path();
            let expect = entity;
            let actual = Entity::new(entity.id(), info.tag);
            return Err(FetchError::Mismatch { expect, actual });
        }
        info.location.ok_or(FetchError::NotSpawned(entity))
    }

    /// Resizes the internal storage to accommodate a new entity index.
    ///
    /// This is a cold path called when an entity index exceeds current capacity.
    /// New slots are initialized with the first tag and no location.
    #[cold]
    #[inline(never)]
    fn resize(&mut self, len: usize) {
        self.infos.reserve(len - self.infos.len());
        self.infos.resize(self.infos.capacity(), DEFAULT_INFO);
    }

    /// Frees an entity slot for reuse.
    ///
    /// This advances the tag counter, making the slot available for
    /// new entities. Any future references to the old tag will fail.
    ///
    /// Usually this is called after despawning an entity so the slot can be recycled.
    ///
    /// # Safety
    /// Caller must ensure:
    /// - The entity is not currently spawned
    /// - The slot is valid for the given ID
    ///
    /// # Returns
    /// The new entity with advanced tag.
    pub unsafe fn free(&mut self, id: EntityId, version: u32) -> Entity {
        let index = id.index();
        if index >= self.infos.len() {
            self.resize(index + 1);
        }

        let info = unsafe { self.infos.get_unchecked_mut(index) };
        debug_assert!(info.location.is_none());

        let (new_tag, wrapping) = info.tag.checked_add(version);

        if wrapping {
            tracing::warn!("Entity({id}) tag wrapped on Entities::free, aliasing may occur.");
        }

        info.tag = new_tag;
        Entity::new(id, new_tag)
    }

    /// Checks if an entity can be spawned.
    ///
    /// # Returns
    /// - `Ok(())` - Entity can be spawned
    /// - `Err(SpawnError)` - If spawning is not possible
    pub fn can_spawn(&self, entity: Entity) -> Result<(), SpawnError> {
        let info = self.infos.get(entity.index()).unwrap_or(&DEFAULT_INFO);

        if info.location.is_some() {
            return Err(SpawnError::AlreadySpawned(entity));
        }
        if info.tag != entity.tag() {
            let expect = entity;
            let actual = Entity::new(entity.id(), info.tag);
            return Err(SpawnError::Mismatch { expect, actual });
        }

        Ok(())
    }

    /// Marks an entity as spawned at the given location.
    ///
    /// # Safety
    /// Caller must ensure:
    /// * The entity was checked with [`Entities::can_spawn`] first
    /// * The location is valid and properly initialized
    ///
    /// # Parameters
    /// * `entity` - The entity being spawned
    /// * `location` - Where the entity's components are stored
    ///
    /// # Returns
    /// * `Ok(())` - Successfully recorded spawn
    /// * `Err(SpawnError)` - If entity state is invalid
    pub unsafe fn set_spawned(
        &mut self,
        entity: Entity,
        location: EntityLocation,
    ) -> Result<(), SpawnError> {
        let index = entity.index();
        if index >= self.infos.len() {
            self.resize(index + 1);
            unsafe {
                self.infos.get_unchecked_mut(index).tag = entity.tag();
            }
        }
        // SAFETY: just resize above.
        let info = unsafe { self.infos.get_unchecked_mut(index) };

        if info.tag != entity.tag() {
            core::hint::cold_path();
            let expect = entity;
            let actual = Entity::new(entity.id(), info.tag);
            return Err(SpawnError::Mismatch { expect, actual });
        }

        if info.location.is_some() {
            core::hint::cold_path();
            return Err(SpawnError::AlreadySpawned(entity));
        }

        info.location = Some(location);
        Ok(())
    }

    /// Marks an entity as despawned and returns its former location.
    ///
    /// # Safety
    /// Caller must ensure the entity is actually being despawned and its
    /// components are properly cleaned up.
    ///
    /// # Returns
    /// - `Ok(EntityLocation)` - The entity's former location
    /// - `Err(DespawnError)` - If entity state is invalid
    pub unsafe fn set_despawned(&mut self, entity: Entity) -> Result<EntityLocation, DespawnError> {
        let Some(info) = self.infos.get_mut(entity.index()) else {
            core::hint::cold_path();
            return Err(DespawnError::NotFound(entity.id()));
        };
        if info.tag != entity.tag() {
            core::hint::cold_path();
            let expect = entity;
            let actual = Entity::new(entity.id(), info.tag);
            return Err(DespawnError::Mismatch { expect, actual });
        }
        info.location.take().ok_or(DespawnError::NotSpawned(entity))
    }

    /// Marks an entity as despawned and returns its former location.
    ///
    /// # Safety
    /// Caller must ensure the entity is actually being despawned and its
    /// components are properly cleaned up.
    pub unsafe fn update_location(
        &mut self,
        entity: Entity,
        location: EntityLocation,
    ) -> Result<(), MoveError> {
        let Some(info) = self.infos.get_mut(entity.index()) else {
            return Err(MoveError::NotFound(entity.id()));
        };
        if info.tag != entity.tag() {
            core::hint::cold_path();
            let expect = entity;
            let actual = Entity::new(entity.id(), info.tag);
            return Err(MoveError::Mismatch { expect, actual });
        }
        let Some(loc) = &mut info.location else {
            core::hint::cold_path();
            return Err(MoveError::NotSpawned(entity));
        };

        *loc = location;
        Ok(())
    }

    /// Updates an entity's location after a move between storages.
    ///
    /// # Safety
    /// Caller must ensure the move actually occurred and the new row is valid.
    ///
    /// # Returns
    /// - `Ok(())` - Location updated successfully
    /// - `Err(MoveError)` - If entity state is invalid
    pub unsafe fn update_row(&mut self, moved: MovedEntityRow) -> Result<(), MoveError> {
        let Some(entity) = moved.entity else {
            return Ok(());
        };
        let Some(info) = self.infos.get_mut(entity.index()) else {
            core::hint::cold_path();
            return Err(MoveError::NotFound(entity.id()));
        };
        if info.tag != entity.tag() {
            core::hint::cold_path();
            let expect = entity;
            let actual = Entity::new(entity.id(), info.tag);
            return Err(MoveError::Mismatch { expect, actual });
        }
        let Some(location) = &mut info.location else {
            core::hint::cold_path();
            return Err(MoveError::NotSpawned(entity));
        };
        match moved.new_row {
            Row::Arche(arche_row) => location.arche_row = arche_row,
            Row::Table(table_row) => location.table_row = table_row,
        }
        Ok(())
    }

    /// Returns an iterator over spawned entities.
    pub fn iter(&self) -> impl FusedIterator<Item = (Entity, EntityLocation)> {
        self.infos.iter().enumerate().filter_map(|(idx, info)| {
            // The location of invalid index `0` must be `None`.
            if let Some(location) = info.location {
                // Faster than `without_provenance` in this hot path.
                let temp = Entity::from_bits(idx as u64);
                let entity = Entity::new(temp.id(), info.tag);
                Some((entity, location))
            } else {
                None
            }
        })
    }
}

// -----------------------------------------------------------------------------
// Update Row

/// Internal enum for specifying which row to update during an entity move.
#[derive(Debug, Clone, Copy)]
enum Row {
    Arche(ArcheRow),
    Table(TableRow),
}

/// Records a change in an entity's storage location.
///
/// This is used internally when entities move between archetypes or
/// within tables, ensuring that entity locations stay in sync with
/// component storage.
#[derive(Debug, Clone, Copy)]
pub struct MovedEntityRow {
    entity: Option<Entity>,
    new_row: Row,
}

impl MovedEntityRow {
    /// Creates a move record for a table row change.
    #[inline(always)]
    pub const fn in_table(entity: Option<Entity>, row: TableRow) -> Self {
        Self {
            entity,
            new_row: Row::Table(row),
        }
    }

    /// Creates a move record for an archetype row change.
    #[inline(always)]
    pub const fn in_arche(entity: Option<Entity>, row: ArcheRow) -> Self {
        Self {
            entity,
            new_row: Row::Arche(row),
        }
    }
}
