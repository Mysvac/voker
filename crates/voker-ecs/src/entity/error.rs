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
pub enum CloneError {
    #[error("Entity with ID {0} was not found during clone operation")]
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

#[derive(Debug, Error, Clone, Copy)]
#[non_exhaustive]
pub enum EntityError {
    #[error("Spawn operation failed: {0}")]
    Spawn(SpawnError),

    #[error("Despawn operation failed: {0}")]
    Despawn(DespawnError),

    #[error("Fetch operation failed: {0}")]
    Fetch(FetchError),

    #[error("Clone operation failed: {0}")]
    Clone(CloneError),

    #[error("Move operation failed: {0}")]
    Move(MoveError),
}

impl EntityError {
    #[cold]
    #[inline(never)]
    pub fn handle_error(&self) -> ! {
        panic!("{self}");
    }
}

macro_rules! impl_from {
    ($name:ident, $variant:ident) => {
        impl TryFrom<EntityError> for $name {
            type Error = EntityError;
            #[inline]
            fn try_from(value: EntityError) -> Result<Self, Self::Error> {
                if let EntityError::$variant(ret) = value {
                    Ok(ret)
                } else {
                    Err(value)
                }
            }
        }

        impl From<$name> for EntityError {
            #[inline]
            fn from(value: $name) -> Self {
                EntityError::$variant(value)
            }
        }

        impl $name {
            #[cold]
            #[inline(never)]
            pub fn handle_error(&self) -> ! {
                panic!("{self}");
            }

            #[inline]
            pub const fn promote(self) -> EntityError {
                EntityError::$variant(self)
            }
        }
    };
}

impl_from!(CloneError, Clone);
impl_from!(FetchError, Fetch);
impl_from!(MoveError, Move);
impl_from!(SpawnError, Spawn);
impl_from!(DespawnError, Despawn);
