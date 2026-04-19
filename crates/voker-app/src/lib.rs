#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, expect(internal_features, reason = "needed for fake_variadic"))]
#![cfg_attr(docsrs, feature(doc_cfg, rustdoc_internals))]
#![forbid(unsafe_code)]
#![no_std]

// -----------------------------------------------------------------------------
// Compilation config

/// Some macros used for compilation control.
pub mod cfg {
    voker_cfg::define_alias! {
        #[cfg(feature = "std")] => std,
    }
}

// -----------------------------------------------------------------------------
// no_std support

crate::cfg::std! { extern crate std; }

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
pub use sub_app::*;
pub use task_pool_plugin::*;

// -----------------------------------------------------------------------------
// terminal_ctrl_c

#[cfg(all(any(all(unix, not(target_os = "horizon")), windows), feature = "std"))]
mod terminal_ctrl_c_handler;

#[cfg(not(all(any(all(unix, not(target_os = "horizon")), windows), feature = "std")))]
mod terminal_ctrl_c_handler {
    #[derive(Default)]
    /// No-op fallback when terminal Ctrl+C handling is unavailable.
    pub struct TerminalCtrlCHandlerPlugin;

    impl Plugin for TerminalCtrlCHandlerPlugin {
        fn build(&self, _: &mut App) {}
    }
}

pub use terminal_ctrl_c_handler::TerminalCtrlCHandlerPlugin;

// -----------------------------------------------------------------------------
// Exports

/// Includes the most common types in this crate.
pub mod prelude {
    #[doc(hidden)]
    pub use crate::{
        App, AppExit, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate,
        Last, Main, Plugin, PluginGroup, PostStartup, PostUpdate, PreStartup, PreUpdate,
        SpawnScene, Startup, SubApp, TaskPoolOptions, TaskPoolPlugin, Update,
    };
}
