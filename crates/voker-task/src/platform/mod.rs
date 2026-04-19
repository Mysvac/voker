use crate::cfg;

mod thread_executor;

cfg::switch! {
    crate::cfg::std => {
        mod multi;
        use multi as impls;
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
