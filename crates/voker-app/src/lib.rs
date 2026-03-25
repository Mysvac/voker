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
pub mod plugin;
pub mod schedule;
pub mod sub_app;

pub use app::App;
pub use plugin::{Plugin, PluginsState};
pub use sub_app::SubApp;
