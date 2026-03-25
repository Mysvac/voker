#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, expect(internal_features, reason = "needed for fake_variadic"))]
#![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
#![expect(unsafe_code, reason = "ECS requires underlying operation")]
#![no_std]

// -----------------------------------------------------------------------------
// Compilation config

/// Some macros used for compilation control.
pub mod cfg {
    voker_cfg::define_alias! {
        #[cfg(feature = "std")] => std,
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
// Modules

pub use voker_ecs_derive as derive;

pub mod borrow;
pub mod bundle;
pub mod error;
pub mod tick;
pub mod utils;

pub mod command;
pub mod component;
pub mod message;
pub mod resource;
pub mod storage;

pub mod archetype;
pub mod entity;

pub mod label;
pub mod query;
pub mod schedule;
pub mod system;

pub mod world;

pub mod name;

/// Re-exported macro internals used by derive-generated code.
pub mod __macro_exports;

// -----------------------------------------------------------------------------
// prelude

pub mod prelude {
    pub use crate::borrow::{Mut, NonSend, NonSendMut, NonSendRef, Ref, Res, ResMut, ResRef};
    pub use crate::bundle::Bundle;
    pub use crate::command::{Commands, EntityCommands};
    pub use crate::component::Component;
    pub use crate::entity::Entity;
    pub use crate::message::{Message, MessageMutator, MessageReader, MessageWriter, Messages};
    pub use crate::query::{Added, And, Changed, Or, Query, With, Without};
    pub use crate::resource::Resource;
    pub use crate::schedule::{Schedule, ScheduleLabel};
    pub use crate::system::{IntoSystem, Local, System};
    pub use crate::tick::{DetectChanges, Tick};
    pub use crate::world::{EntityMut, EntityOwned, EntityRef, World};
}
