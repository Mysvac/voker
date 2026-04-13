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

pub use exit::AppExit;
pub use label::{AppLabel, InternedAppLabel};

pub mod app;
pub mod main_schedule;
pub mod plugin;
pub mod sub_app;

pub use app::{App, SubApps};
pub use main_schedule::MainSchedulePlugin;
pub use plugin::{Plugin, Plugins, PluginsState};
pub use sub_app::SubApp;
