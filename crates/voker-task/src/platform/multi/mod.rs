
// -----------------------------------------------------------------------------
// Modules

mod task;
mod xor_shift;
mod scope_executor;
mod task_pool;
mod global_executor;

// -----------------------------------------------------------------------------
// Internal API

use xor_shift::XorShift64Star;

use global_executor::GlobalExecutor;

use super::local_executor::LocalExecutor;

// -----------------------------------------------------------------------------
// Exports

pub use task::Task;
pub use scope_executor::{ScopeExecutor, ScopeExecutorTicker};
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

/// A function used to tick the global tasks pools on the main thread.
/// 
/// This will run a maximum of 100 local tasks per executor per call to this function.
///
/// # Warning
///
/// This function *must* be called on the main thread, or the task pools will not be updated appropriately.
pub fn tick_local_executor_on_main_thread() {
    COMPUTE_TASK_POOL
        .get()
        .unwrap()
        .with_local_executor(|compute_local_executor| {
            ASYNC_COMPUTE_TASK_POOL
                .get()
                .unwrap()
                .with_local_executor(|async_local_executor| {
                    IO_TASK_POOL
                        .get()
                        .unwrap()
                        .with_local_executor(|io_local_executor| {
                            for _ in 0..100 {
                                compute_local_executor.try_tick();
                                async_local_executor.try_tick();
                                io_local_executor.try_tick();
                            }
                        });
                });
        });
}
