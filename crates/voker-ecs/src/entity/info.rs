use alloc::vec::Vec;
use core::fmt::Debug;

use crate::archetype::{ArcheId, ArcheRow};
use crate::entity::error::{DespawnError, FetchError, MoveError, SpawnError};
use crate::entity::{Entity, EntityError, EntityGeneration, EntityId};
use crate::storage::{TableId, TableRow};

// -----------------------------------------------------------------------------
// EntityToken

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
    generation: EntityGeneration,
    location: Option<EntityLocation>,
}

// -----------------------------------------------------------------------------
// Entities

/// Central registry for all entity metadata in the ECS world.
///
/// `Entities` maintains a sparse set of all entity IDs that have ever been
/// allocated, tracking their current generation and storage location. It
/// provides methods for spawning, despawning, and locating entities while
/// ensuring type safety through generation counters.
///
/// # Generations
///
/// Each entity slot has a generation counter that increments when the entity
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
        fn entity_from(id: usize, generation: EntityGeneration) -> Entity {
            let id = Entity::from_bits(id as u64).id();
            Entity::new(id, generation)
        }

        f.debug_list()
            .entries(
                self.infos
                    .iter()
                    .enumerate()
                    .filter(|(_, info)| info.location.is_some())
                    .map(|(id, info)| entity_from(id, info.generation)),
            )
            .finish()
    }
}

impl Entities {
    /// Creates a new empty entity registry.
    pub(crate) const fn new() -> Self {
        Self { infos: Vec::new() }
    }

    /// Return the number of spawned entities.
    ///
    /// # Complexity
    /// time: O(N)
    pub fn len(&self) -> usize {
        self.infos
            .iter()
            .filter(|info| info.location.is_some())
            .count()
    }

    /// Return true if there are no spawned entities.
    ///
    /// # Complexity
    /// time: O(N)
    pub fn is_empty(&self) -> bool {
        self.infos.iter().all(|info| info.location.is_none())
    }

    /// Resolves an entity ID to its current entity with correct generation.
    ///
    /// # Complexity
    /// time: O(1)
    pub fn resolve(&self, id: EntityId) -> Entity {
        if let Some(info) = self.infos.get(id.index()) {
            Entity::new(id, info.generation)
        } else {
            Entity::from_id(id)
        }
    }

    /// Retrieves the location of a spawned entity.
    ///
    /// # Returns
    /// - `Ok(EntityLocation)` - The entity's current storage location
    /// - `Err(EntityError)` - If the entity doesn't exist, generation mismatches,
    ///   or the entity is not spawned
    ///
    /// # Errors
    /// - `FetchError::NotFound` - Entity index out of bounds
    /// - `FetchError::Mismatch` - Generation counter mismatch (stale entity)
    /// - `FetchError::NotSpawned` - Entity exists but is not spawned
    pub fn locate(&self, entity: Entity) -> Result<EntityLocation, EntityError> {
        let Some(info) = self.infos.get(entity.index()) else {
            return Err(FetchError::NotFound(entity.id()).into());
        };
        if info.generation != entity.generation() {
            return Err(FetchError::Mismatch {
                expect: entity,
                actual: Entity::new(entity.id(), info.generation),
            }
            .into());
        }
        info.location.ok_or(FetchError::NotSpawned(entity).into())
    }

    /// Resizes the internal storage to accommodate a new entity index.
    ///
    /// This is a cold path called when an entity index exceeds current capacity.
    /// New slots are initialized with the first generation and no location.
    #[cold]
    #[inline(never)]
    fn resize(&mut self, len: usize) {
        self.infos.reserve(len - self.infos.len());
        self.infos.resize(
            self.infos.capacity(),
            const {
                EntityInfo {
                    generation: EntityGeneration::FIRST,
                    location: None,
                }
            },
        );
    }

    /// Frees an entity slot for reuse.
    ///
    /// This advances the generation counter, making the slot available for
    /// new entities. Any future references to the old generation will fail.
    ///
    /// Useually, we will call this function after despawn a entity, then we
    /// can recycle it.
    ///
    /// # Safety
    /// Caller must ensure:
    /// - The entity is not currently spawned
    /// - The slot is valid for the given ID
    ///
    /// # Returns
    /// The new entity with advanced generation.
    pub unsafe fn free(&mut self, id: EntityId, generation: u32) -> Entity {
        let index = id.index();
        if index >= self.infos.len() {
            self.resize(index + 1);
        }

        let info = unsafe { self.infos.get_unchecked_mut(index) };
        debug_assert!(info.location.is_none());

        let (new_gen, wrapping) = info.generation.checked_add(generation);
        info.generation = new_gen;
        if wrapping {
            log::warn!("Entity({id}) generation wrapped on Entities::free, aliasing may occur.");
        }

        Entity::new(id, new_gen)
    }

    /// Checks if an entity can be spawned.
    ///
    /// # Returns
    /// - `Ok(())` - Entity can be spawned
    /// - `Err(EntityError::SpawnError)` - If spawning is not possible
    pub fn can_spawn(&self, entity: Entity) -> Result<(), EntityError> {
        let index = entity.index();

        let Some(info) = self.infos.get(index) else {
            return Ok(());
        };

        if info.location.is_some() {
            return Err(SpawnError::AlreadySpawned(entity).into());
        }
        if info.generation != entity.generation() {
            return Err(SpawnError::Mismatch {
                expect: entity,
                actual: Entity::new(entity.id(), info.generation),
            }
            .into());
        }

        Ok(())
    }

    /// Marks an entity as spawned at the given location.
    ///
    /// # Safety
    /// Caller must ensure:
    /// * The entity was checked with `can_spawned` first
    /// * The location is valid and properly initialized
    ///
    /// # Parameters
    /// * `entity` - The entity being spawned
    /// * `location` - Where the entity's components are stored
    ///
    /// # Returns
    /// * `Ok(())` - Successfully recorded spawn
    /// * `Err(EntityError::SpawnError)` - If entity state is invalid
    pub unsafe fn set_spawned(
        &mut self,
        entity: Entity,
        location: EntityLocation,
    ) -> Result<(), EntityError> {
        let index = entity.index();
        if index >= self.infos.len() {
            self.resize(index + 1);
            let info = unsafe { self.infos.get_unchecked_mut(index) };
            info.generation = entity.generation();
        }

        let info = unsafe { self.infos.get_unchecked_mut(index) };
        if info.generation != entity.generation() {
            return Err(SpawnError::Mismatch {
                expect: entity,
                actual: Entity::new(entity.id(), info.generation),
            }
            .into());
        }
        if info.location.is_some() {
            return Err(SpawnError::AlreadySpawned(entity).into());
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
    /// - `Err(EntityError)` - If entity state is invalid
    pub unsafe fn set_despawned(&mut self, entity: Entity) -> Result<EntityLocation, EntityError> {
        let Some(info) = self.infos.get_mut(entity.index()) else {
            return Err(DespawnError::NotFound(entity.id()).into());
        };
        if info.generation != entity.generation() {
            return Err(DespawnError::Mismatch {
                expect: entity,
                actual: Entity::new(entity.id(), info.generation),
            }
            .into());
        }
        info.location
            .take()
            .ok_or(DespawnError::NotSpawned(entity).into())
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
    ) -> Result<(), EntityError> {
        let Some(info) = self.infos.get_mut(entity.index()) else {
            return Err(MoveError::NotFound(entity.id()).into());
        };
        if info.generation != entity.generation() {
            return Err(MoveError::Mismatch {
                expect: entity,
                actual: Entity::new(entity.id(), info.generation),
            }
            .into());
        }
        if let Some(l) = &mut info.location {
            *l = location;
            Ok(())
        } else {
            Err(MoveError::NotSpawned(entity).into())
        }
    }

    /// Updates an entity's location after a move between storages.
    ///
    /// # Safety
    /// Caller must ensure the move actually occurred and the new row is valid.
    ///
    /// # Returns
    /// - `Ok(())` - Location updated successfully
    /// - `Err(EntityError)` - If entity state is invalid
    pub unsafe fn update_row(&mut self, moved: MovedEntityRow) -> Result<(), EntityError> {
        let Some(entity) = moved.entity else {
            return Ok(());
        };

        let Some(info) = self.infos.get_mut(entity.index()) else {
            return Err(MoveError::NotFound(entity.id()).into());
        };
        if info.generation != entity.generation() {
            return Err(MoveError::Mismatch {
                expect: entity,
                actual: Entity::new(entity.id(), info.generation),
            }
            .into());
        }
        let Some(location) = &mut info.location else {
            return Err(MoveError::NotSpawned(entity).into());
        };
        match moved.new_row {
            Row::Arche(arche_row) => location.arche_row = arche_row,
            Row::Table(table_row) => location.table_row = table_row,
        }
        Ok(())
    }

    pub fn iter(&self) -> impl Iterator<Item = (Entity, EntityLocation)> {
        self.infos.iter().enumerate().filter_map(|(idx, info)| {
            if let Some(location) = info.location {
                let temp = Entity::from_bits(idx as u64);
                let entity = Entity::new(temp.id(), info.generation);
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
