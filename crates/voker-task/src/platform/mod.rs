use crate::cfg;

mod local_executor;

cfg::switch! {
    cfg::web => {
        mod web;
        use web as impls;
    }
    cfg::std => {
        mod multi;
        use multi as impls;
    }
    _ => {
        mod fallback;
        use fallback as impls;
    }
}

pub use impls::tick_local_executor_on_main_thread;
pub use impls::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool};
pub use impls::{Scope, TaskPool, TaskPoolBuilder};
pub use impls::{ScopeExecutor, ScopeExecutorTicker};
pub use impls::{Task, block_on};
