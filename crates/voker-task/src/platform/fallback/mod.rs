//! A single-thread Task Pool for no_std env.
//!
//! **Important**: Can only be used in main thread,
//! because this is a single-thread task pool.

// -----------------------------------------------------------------------------
// Modules

mod global_executor;
mod scope_executor;
mod task;
mod task_pool;

// -----------------------------------------------------------------------------
// Internal API

use global_executor::GlobalExecutor;

use super::local_executor::LocalExecutor;

// -----------------------------------------------------------------------------
// Exports

pub use scope_executor::{ScopeExecutor, ScopeExecutorTicker};
pub use task::Task;
pub use task_pool::{Scope, TaskPool, TaskPoolBuilder};

// -----------------------------------------------------------------------------
// block_on

/// Blocks on the supplied `future`.
/// This implementation will busy-wait until it is completed.
/// Consider enabling the `async-io` or `futures-lite` features.
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    use core::task::{Context, Poll};

    // Pin the future on the stack.
    let mut future = core::pin::pin!(future);
    // We don't care about the waker as we're just going to poll as fast as possible.
    let cx = &mut Context::from_waker(core::task::Waker::noop());

    // Keep polling until the future is ready.
    loop {
        match future.as_mut().poll(cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => core::hint::spin_loop(),
        }
    }
}

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
