//! Bundle composition for batched component insertion.
//!
//! Bundles group multiple components into one spawn/insert operation.
//! They are commonly used for entity initialization and archetype transitions.

// -----------------------------------------------------------------------------
// Modules

mod bundle;
mod bundles;
mod ident;
mod info;

// -----------------------------------------------------------------------------
// Exports

pub use voker_ecs_derive::Bundle;

pub use bundle::Bundle;
pub use bundles::Bundles;
pub use ident::BundleId;
pub use info::BundleInfo;
