#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

// -----------------------------------------------------------------------------
// Compilation config

/// Some macros used for compilation control.
pub mod cfg {
    pub(crate) use voker_cfg::switch;

    voker_cfg::define_alias! {
        #[cfg(feature = "std")] => std,
    }
}

// -----------------------------------------------------------------------------
// no_std support

cfg::std! { extern crate std; }

extern crate alloc;

// -----------------------------------------------------------------------------
// Modules

pub mod sync;
pub mod thread;
pub mod time;
pub mod utils;

// -----------------------------------------------------------------------------
// Special platform support
