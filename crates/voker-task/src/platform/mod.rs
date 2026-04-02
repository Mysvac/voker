use crate::cfg;

mod thread_executor;

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

pub use thread_executor::{ThreadExecutor, ThreadExecutorTicker};

pub use impls::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool};
pub use impls::{Scope, TaskPool, TaskPoolBuilder};
pub use impls::{Task, block_on};
