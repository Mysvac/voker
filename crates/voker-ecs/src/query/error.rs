use thiserror::Error;
use voker_ecs_derive::GameError;

use crate::entity::Entity;

// -----------------------------------------------------------------------------
// QueryEntityError

/// Errors produced by entity-targeted query APIs like `get` and `get_many_mut`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, GameError)]
#[game_error(severity = "info")]
pub enum QueryEntityError {
    /// The entity does not currently exist in the world (despawned, stale, or invalid).
    #[error("Entity {0} does not exist in this world")]
    NoSuchEntity(Entity),

    /// The entity exists, but does not match query data/filter constraints.
    #[error("Entity {0} does not satisfy this query")]
    QueryMismatch(Entity),

    /// Duplicate entity in mutable many-access APIs.
    #[error("Entity {0} appears more than once in mutable many-query access")]
    DuplicateEntity(Entity),
}

// -----------------------------------------------------------------------------
// QuerySingleError

/// Errors produced by single-target query APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, GameError)]
#[game_error(severity = "warning")]
pub enum QuerySingleError {
    /// No entity matched the query.
    #[error("No entities match this query")]
    NoEntities,

    /// More than one entity matched the query.
    #[error("More than one entity matches this query")]
    MultipleEntities,
}
