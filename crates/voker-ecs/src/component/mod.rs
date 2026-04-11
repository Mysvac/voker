//! Component traits, metadata, and registration.
//!
//! Components are entity-attached data units. This module provides:
//! - the [`Component`] trait and derive re-export,
//! - runtime metadata/descriptor types,
//! - component registry and required-component support,
//! - storage strategy selection (`dense` vs `sparse`).

// -----------------------------------------------------------------------------
// Modules

mod component;
mod components;
mod hook;
mod ident;
mod info;
mod required;
mod storage;
mod tools;

// -----------------------------------------------------------------------------
// Exports

pub use voker_ecs_derive::Component;

pub use component::Component;
pub use components::Components;
pub use hook::{ComponentHook, ComponentHooks, HookContext};
pub use ident::ComponentId;
pub use info::{ComponentDescriptor, ComponentInfo};
pub use required::{Required, RequiredComponents};
pub use storage::StorageMode;
pub use tools::{CollectResult, ComponentCollector};
pub use tools::{ComponentRegistrar, ComponentWriter};
