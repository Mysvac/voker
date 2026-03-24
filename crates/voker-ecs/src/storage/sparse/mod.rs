//! Sparse storage for components that are rare or irregularly distributed.
//!
//! Sparse maps trade iteration locality for lower memory overhead when most
//! entities do not carry a given component.

// -----------------------------------------------------------------------------
// Module

mod ident;
mod map;
mod maps;

// -----------------------------------------------------------------------------
// Exports

pub use ident::{MapId, MapRow};
pub use map::Map;
pub use maps::Maps;
