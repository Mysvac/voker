//! Component traits, metadata, and registration.
//!
//! Components are entity-attached data units. This module provides:
//! - the [`Component`] trait and derive re-export,
//! - runtime metadata/descriptor types,
//! - component registry and required-component support,
//! - storage strategy selection (`dense` vs `sparse`).

// -----------------------------------------------------------------------------
// Modules

mod components;
mod ident;
mod impls;
mod info;
mod required;
mod storage;
mod tools;

// -----------------------------------------------------------------------------
// Exports

pub use voker_ecs_derive::Component;

pub use components::Components;
pub use ident::ComponentId;
pub use impls::Component;
pub use info::{ComponentDescriptor, ComponentInfo};
pub use required::{Required, RequiredComponents};
pub use storage::ComponentStorage;
pub use tools::*;
