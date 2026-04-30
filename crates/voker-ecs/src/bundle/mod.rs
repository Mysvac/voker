//! Bundle composition for batched component insertion.
//!
//! A [`Bundle`] groups one or more component types so they can be inserted or
//! removed in a single operation. The derive macro implements the trait for
//! structs and handles both explicit (directly listed) and required (via
//! `#[require]`) component sets.
//!
//! At runtime, [`Bundles`] caches compiled [`BundleInfo`] keyed by `TypeId` so
//! repeated insert/remove patterns pay the reflection cost only once.

// -----------------------------------------------------------------------------
// Modules

mod bundle;
mod bundles;
mod ident;
mod info;

// -----------------------------------------------------------------------------
// Exports

pub use voker_ecs_derive::Bundle;

pub use bundle::{Bundle, DataBundle};
pub use bundles::Bundles;
pub use ident::BundleId;
pub use info::BundleInfo;
