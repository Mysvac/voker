//! Low-level world access adapters.
//!
//! These types expose entity-centric and pointer-centric access paths used by
//! query/system internals:
//! - [`EntityOwned`]/[`EntityRef`]/[`EntityMut`]: entity views,
//! - [`GetComponents`]/[`FetchComponents`]: generic component access traits.

// -----------------------------------------------------------------------------
// Modules

mod entity;
mod fetch_component;
mod get_component;
mod insert;
mod remove;

// -----------------------------------------------------------------------------
// Exports

pub use entity::{EntityMut, EntityOwned, EntityRef};
pub use fetch_component::FetchComponents;
pub use get_component::GetComponents;
