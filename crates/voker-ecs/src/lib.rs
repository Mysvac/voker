#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, expect(internal_features, reason = "needed for fake_variadic"))]
#![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
#![expect(unsafe_code, reason = "ECS requires underlying operation")]
#![expect(clippy::missing_safety_doc, reason = "refactor...")]
#![no_std]

// -----------------------------------------------------------------------------
// Compilation config

/// Some macros used for compilation control.
pub mod cfg {
    voker_cfg::define_alias! {
        #[cfg(feature = "std")] => std,
        #[cfg(feature = "backtrace")] => backtrace,
        #[cfg(any(feature = "debug", debug_assertions))] => debug,
    }
}

// -----------------------------------------------------------------------------
// Extern Self

// Usually, we need to use `crate` in the crate itself and use `voker_ecs` in doc testing.
// But `macro_utils::Manifest` can only choose one, so we must have an
// `extern self` to ensure `voker_ecs` can be used as an alias for `crate`.
extern crate self as voker_ecs;

// -----------------------------------------------------------------------------
// no_std support

crate::cfg::std! { extern crate std; }

extern crate alloc;

// -----------------------------------------------------------------------------
// Proc-macro re-export

pub use voker_ecs_derive as derive;

// ==========================================
// Layer 1 — Primitives
// ==========================================
//
// Foundation types used by every other layer.

pub mod borrow;
pub mod error;
pub mod label;
pub mod tick;
pub mod utils;

// ==========================================
// Layer 2 — Core ECS Data Model
// ==========================================
//
// Entities, components, archetypes, and all storage backends.

pub mod archetype;
pub mod bundle;
pub mod component;
pub mod entity;
pub mod resource;
pub mod storage;

// ==========================================
// Layer 3 — World
// ==========================================
//
// The `World` owns all ECS state and provides the high-level API for
// structural mutations (spawn, insert, remove, despawn) and resource access.

pub mod world;

// ==========================================
// Layer 4 — Systems
// ==========================================
//
// Systems are the unit of logic in the ECS. This layer includes system
// functions, parameters, access tracking, and scheduling.

pub mod command;
pub mod query;
pub mod schedule;
pub mod system;

// ==========================================
// Layer 5 — Events, Observers, and Messaging
// ==========================================
//
// Reactive event dispatch, observer registration, and typed message channels.

pub mod event;
pub mod message;
pub mod observer;
pub mod spawn;

// ==========================================
// Layer 6 — Higher-Level Abstractions
// ==========================================
//
// Optional but commonly used features built on the layers above.

pub mod clone;
pub mod hierarchy;
pub mod reflect;
pub mod relationship;
pub mod template;

pub mod name;
pub mod traversal;

// ==========================================
// Prelude
// ==========================================

pub mod prelude {
    // --- Primitives ---
    pub use crate::borrow::{Mut, NonSend, NonSendMut, NonSendRef, Ref, Res, ResMut, ResRef};
    pub use crate::error::{ErrorContext, ErrorHandler, FallbackErrorHandler};
    pub use crate::error::{GameError, IntoGameError};
    pub use crate::tick::{DetectChanges, Tick};

    // --- Core ECS ---
    pub use crate::bundle::{Bundle, BundleId};
    pub use crate::component::{Component, ComponentHook, ComponentId, HookContext};
    pub use crate::entity::{Entity, EntityLocation, EntityMapper, MapEntities};
    pub use crate::entity::{EntityHashMap, EntityHashSet, EntityIndexMap, EntityIndexSet};
    pub use crate::resource::{Resource, ResourceId};

    // --- World ---
    pub use crate::world::{DeferredWorld, EntityMut, EntityOwned, EntityRef, World};

    // --- Systems ---
    pub use crate::command::{Command, Commands, EntityCommands};
    pub use crate::query::{Added, And, Changed, Or, With, Without};
    pub use crate::query::{Query, QueryEntityError, QuerySingleError, QueryState, Single};
    pub use crate::schedule::{IntoSystemConfig, Schedule, ScheduleLabel, SystemSet};
    pub use crate::system::{IntoSystem, Local, System, SystemError, SystemId};

    // --- Events & Observers ---
    pub use crate::event::{Add, Clone, Despawn, Discard, Insert, Remove};
    pub use crate::event::{EntityEvent, EntityEventMut, Event};
    pub use crate::message::{Message, MessageKey, MessageMutator, MessageReader, MessageWriter};
    pub use crate::observer::{IntoEntityObserver, IntoObserver, ObservedBy, On};

    // --- Higher-Level ---
    pub use crate::hierarchy::{ChildOf, Children};
    pub use crate::name::Name;
    pub use crate::reflect::{AppTypeRegistry, ReflectFromWorld, ReflectMapEntities};
    pub use crate::reflect::{ReflectComponent, ReflectResource};
    pub use crate::relationship::{RelatedSpawner, RelatedSpawnerCommands};
    pub use crate::relationship::{Relationship, RelationshipTarget};
    pub use crate::traversal::Traversal;
}
