#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

// -----------------------------------------------------------------------------
// Compilation config

pub mod cfg {
    pub(crate) use voker_cfg::switch;

    voker_cfg::define_alias! {
        #[cfg(feature = "std")] => std,
        #[cfg(all(feature = "std", feature = "async_io"))] => async_io,
        #[cfg(not(feature = "std"))] => single_threaded,
        #[cfg(feature = "std")] => multi_threaded,
    }
}

// -----------------------------------------------------------------------------
// no_std support

cfg::std! {
    extern crate std;
}

extern crate alloc;

// -----------------------------------------------------------------------------
// Modules

mod macro_utils;
mod slice;

pub mod futures;
mod platform;

// -----------------------------------------------------------------------------
// Exports

pub use slice::ParallelSlice;

pub use platform::block_on;
pub use platform::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool};
pub use platform::{Scope, TaskPool, TaskPoolBuilder};
pub use platform::{ThreadExecutor, ThreadExecutorTicker};

// -----------------------------------------------------------------------------
// Re-Exports

pub use async_task::Task;
pub use futures_lite;
pub use futures_lite::future::poll_once;
