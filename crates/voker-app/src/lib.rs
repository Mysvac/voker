#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, expect(internal_features, reason = "needed for fake_variadic"))]
#![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
#![forbid(unsafe_code)]
#![no_std]

// -----------------------------------------------------------------------------
// no_std support

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

// -----------------------------------------------------------------------------
// modules

mod exit;
mod label;

mod app;
mod plugin;
mod sub_app;

mod main_schedule;
mod panic_handler;
mod schedule_runner;
mod shutdown;
mod task_pool_plugin;

// -----------------------------------------------------------------------------
// Exports

pub use exit::AppExit;
pub use label::{AppLabel, InternedAppLabel};

pub use app::*;
pub use main_schedule::*;
pub use panic_handler::*;
pub use plugin::*;
pub use schedule_runner::*;
pub use shutdown::*;
pub use sub_app::*;
pub use task_pool_plugin::*;

pub use voker_app_derive as derive;
pub use voker_app_derive::{AppLabel, voker_main};

// -----------------------------------------------------------------------------
// Exports

/// Includes the most common types in this crate.
pub mod prelude {
    pub use crate::{App, AppExit, Plugin, PluginGroup, Plugins, SubApp};
    pub use crate::{First, Last, PostStartup, PreStartup, SpawnScene, Startup};
    pub use crate::{FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate};
    pub use crate::{PostUpdate, PreUpdate, RunFixedMainLoopSystems, Update};
    pub use crate::{TaskPoolOptions, TaskPoolPlugin, voker_main};
}

pub mod exports {
    voker_os::cfg::android! {
        use voker_os::sync::OnceLock;
        pub use voker_os::exports::android_activity;
        pub static ANDROID_APP: OnceLock<android_activity::AndroidApp> = OnceLock::new();
    }
}
