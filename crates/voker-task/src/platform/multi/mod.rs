
// -----------------------------------------------------------------------------
// Modules

mod task;
mod xor_shift;
mod task_pool;
mod global_executor;

// -----------------------------------------------------------------------------
// Internal API

use super::{ThreadExecutor, ThreadExecutorTicker};
use xor_shift::XorShift64Star;
use global_executor::GlobalExecutor;

// -----------------------------------------------------------------------------
// Exports

pub use task::Task;
pub use task_pool::{TaskPool, TaskPoolBuilder, Scope};

// -----------------------------------------------------------------------------
// block_on

crate::cfg::async_io!{
    if {
        pub use async_io::block_on;
    } else {
        pub use futures_lite::future::block_on;
    }
}

// -----------------------------------------------------------------------------
// task_pools

use crate::macro_utils::taskpool;

taskpool! {
    /// A newtype for a task pool for CPU-intensive work that must be completed to
    /// deliver the next frame
    ///
    /// See [`TaskPool`] documentation for details.
    /// 
    /// [`AsyncComputeTaskPool`] should be preferred if the work does not have to be
    /// completed before the next frame.
    (COMPUTE_TASK_POOL, ComputeTaskPool)
}

taskpool! {
    /// A newtype for a task pool for CPU-intensive work that may span across multiple frames
    ///
    /// See [`TaskPool`] documentation for details.
    /// 
    /// Use [`ComputeTaskPool`] if the work must be complete before advancing to the next frame.
    (ASYNC_COMPUTE_TASK_POOL, AsyncComputeTaskPool)
}

taskpool! {
    /// A newtype for a task pool for IO-intensive work.
    /// (i.e. tasks that spend very little time in a "woken" state)
    ///
    /// See [`TaskPool`] documentation for details.
    (IO_TASK_POOL, IoTaskPool)
}
