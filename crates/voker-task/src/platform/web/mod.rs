
// -----------------------------------------------------------------------------
// Modules

mod task;
mod scope_executor;
mod task_pool;

// -----------------------------------------------------------------------------
// Internal API

use super::local_executor::LocalExecutor;

// -----------------------------------------------------------------------------
// Exports

pub use task::Task;
pub use scope_executor::{ScopeExecutor, ScopeExecutorTicker};
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

/// A function used to tick the global tasks pools on the main thread.
/// This will run a maximum of 100 local tasks per executor per call to this function.
///
/// # Warning
///
/// This function *must* be called on the main thread, or the task pools will not be updated appropriately.
/// 
/// # Behavior
/// 
/// In wasm, this function do nothing.
pub fn tick_local_executor_on_main_thread() {
    // do-nothing
}

