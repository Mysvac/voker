// -----------------------------------------------------------------------------
// Modules

mod task;
mod task_pool;

// -----------------------------------------------------------------------------
// Internal API

use super::thread_executor::{ThreadExecutor, ThreadExecutorTicker};

// -----------------------------------------------------------------------------
// Exports

pub use task::Task;
pub use task_pool::{Scope, TaskPool, TaskPoolBuilder};

// -----------------------------------------------------------------------------
// block_on

pub use futures_lite::future::block_on;

// -----------------------------------------------------------------------------
// task_pools

use crate::macro_utils::taskpool;

taskpool! {
    /// A newtype for a task pool for CPU-intensive work that must be completed to
    /// deliver the next frame
    ///
    /// See [`TaskPool`] documentation for details on Bevy tasks.
    /// [`AsyncComputeTaskPool`] should be preferred if the work does not have to be
    /// completed before the next frame.
    (COMPUTE_TASK_POOL, ComputeTaskPool)
}

taskpool! {
    /// A newtype for a task pool for CPU-intensive work that may span across multiple frames
    ///
    /// See [`TaskPool`] documentation for details on Bevy tasks.
    /// Use [`ComputeTaskPool`] if the work must be complete before advancing to the next frame.
    (ASYNC_COMPUTE_TASK_POOL, AsyncComputeTaskPool)
}

taskpool! {
    /// A newtype for a task pool for IO-intensive work (i.e. tasks that spend very little time in a
    /// "woken" state)
    ///
    /// See [`TaskPool`] documentation for details on Bevy tasks.
    (IO_TASK_POOL, IoTaskPool)
}
