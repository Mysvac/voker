use crate::cfg;

mod thread_executor;

cfg::switch! {
    crate::cfg::multi_threaded => {
        mod multi;
        use multi as impls;
    }
    #[cfg(all(feature = "std", target_arch = "wasm32"))] => {
        mod web;
        use web as impls;
    }
    _ => {
        mod fallback;
        use fallback as impls;
    }
}

pub use thread_executor::{ThreadExecutor, ThreadExecutorTicker};

pub use impls::block_on;
pub use impls::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool};
pub use impls::{Scope, TaskPool, TaskPoolBuilder};
