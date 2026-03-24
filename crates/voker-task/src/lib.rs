#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

// -----------------------------------------------------------------------------
// Compilation config

pub mod cfg {
    pub(crate) use voker_cfg::switch;

    voker_cfg::define_alias! {
        #[cfg(feature = "std")] => std,
        #[cfg(all(target_arch = "wasm32", feature = "web"))] => web,
        #[cfg(all(feature = "std", feature = "async_io"))] => async_io,
        #[cfg(all(feature = "std", not(feature = "web")))] => multi_thread,
        #[cfg(any(not(feature = "std"), feature = "web"))] => single_thread,
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

mod cond_send;
mod macro_utils;

mod platform;

mod iter;
mod slice;

pub mod futures;

// -----------------------------------------------------------------------------
// Exports

pub use cond_send::{BoxedFuture, CondSendFuture};

pub use platform::tick_local_executor_on_main_thread;
pub use platform::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool};
pub use platform::{Scope, TaskPool, TaskPoolBuilder};
pub use platform::{ScopeExecutor, ScopeExecutorTicker};
pub use platform::{Task, block_on};

pub use iter::ParallelIterator;
pub use slice::{ParallelSlice, ParallelSliceMut};

// -----------------------------------------------------------------------------
// Re-Exports

pub use futures_lite;
pub use futures_lite::future::poll_once;
