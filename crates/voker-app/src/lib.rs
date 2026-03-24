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

pub mod app;
pub mod plugin;
pub mod schedule;

mod label;

pub use app::App;
pub use label::{AppLabel, InternedAppLabel};
pub use plugin::{Plugin, PluginsState};
