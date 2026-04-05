//! Dense table storage used by archetype-aligned components.
//!
//! Dense storage keeps entity rows and component columns tightly packed,
//! which improves iteration locality for common query patterns.

// -----------------------------------------------------------------------------
// Module

mod ident;
mod table;
mod tables;

// -----------------------------------------------------------------------------
// Exports

pub use ident::{TableCol, TableId, TableRow};
pub use table::Table;
pub use tables::Tables;
