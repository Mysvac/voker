//! Query types, state, iteration, and filters.
//!
//! Query execution is split into two stages:
//! - storage-level filtering (archetype/table selection), then
//! - optional entity-level filtering.
//!
//! [`QueryData`] defines what is fetched (`&T`, `&mut T`, wrappers, tuples),
//! while [`QueryFilter`] defines which entities are included (`With`, `Without`,
//! `Added`, `Changed`, logical `And`/`Or`).
//!
//! [`QueryState`] caches compiled filter/data state and supports incremental
//! updates as archetypes are added.

// -----------------------------------------------------------------------------
// Modules

mod data;
mod filter;
mod iter;
mod query;
mod state;

// -----------------------------------------------------------------------------
// Exports

pub use data::{QueryData, ReadOnlyQueryData};
pub use filter::{Added, And, Changed, Or, QueryFilter, With, Without};
pub use iter::QueryIter;
pub use query::Query;
pub use state::QueryState;
