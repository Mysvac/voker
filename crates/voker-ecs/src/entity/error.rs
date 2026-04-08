use thiserror::Error;

use crate::entity::{Entity, EntityId};

// -----------------------------------------------------------------------------
// Error

#[derive(Debug, Error, Clone, Copy)]
#[non_exhaustive]
pub enum FetchError {
    #[error("Entity with ID {0} was not found during fetch operation")]
    NotFound(EntityId),

    #[error("Entity {0} has not been spawned yet")]
    NotSpawned(Entity),

    #[error("Entity mismatch: expected {expect:?}, found {actual:?}")]
    Mismatch { expect: Entity, actual: Entity },
}

#[derive(Debug, Error, Clone, Copy)]
#[non_exhaustive]
pub enum MoveError {
    #[error("Entity with ID {0} was not found during move operation")]
    NotFound(EntityId),

    #[error("Entity {0} has not been spawned yet")]
    NotSpawned(Entity),

    #[error("Entity mismatch during move: expected {expect:?}, found {actual:?}")]
    Mismatch { expect: Entity, actual: Entity },
}

#[derive(Debug, Error, Clone, Copy)]
#[non_exhaustive]
pub enum DespawnError {
    #[error("Entity with ID {0} was not found during despawn operation")]
    NotFound(EntityId),

    #[error("Entity {0} has not been spawned yet")]
    NotSpawned(Entity),

    #[error("Entity mismatch during despawn: expected {expect:?}, found {actual:?}")]
    Mismatch { expect: Entity, actual: Entity },
}

#[derive(Debug, Error, Clone, Copy)]
#[non_exhaustive]
pub enum SpawnError {
    #[error("Entity with ID {0} was not found during spawn operation")]
    NotFound(EntityId),

    #[error("Entity {0} has already been spawned")]
    AlreadySpawned(Entity),

    #[error("Entity mismatch during spawn: expected {expect:?}, found {actual:?}")]
    Mismatch { expect: Entity, actual: Entity },
}
