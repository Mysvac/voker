//! Storage backends for components and resources.
//!
//! Storage is split into:
//! - dense table storage for archetype-aligned component columns,
//! - sparse map storage for sparse/optional component layouts,
//! - global resource storage,
//! - shared low-level column primitives.
//!
//! Query and world systems choose storage paths based on component metadata and
//! query requirements.

// -----------------------------------------------------------------------------
// Modules

mod column;
mod dense;
mod global;
mod impls;
mod sparse;
mod utils;

// -----------------------------------------------------------------------------
// Internal

use utils::{AbortOnPanic, VecRemoveExt};

// -----------------------------------------------------------------------------
// Exports

pub use column::Column;
pub use dense::{Table, Tables};
pub use dense::{TableCol, TableId, TableRow};
pub use global::{ResourceData, ResourceStorage};
pub use impls::Storages;
pub use sparse::{Map, Maps};
pub use sparse::{MapId, MapRow};
