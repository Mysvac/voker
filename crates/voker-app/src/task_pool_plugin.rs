use crate::{App, Plugin};

use core::fmt::Debug;
use voker_ecs::system::NonSendMarker;
use voker_os::Arc;
use voker_task::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool};
use voker_task::{TaskPool, TaskPoolBuilder};

// -----------------------------------------------------------------------------
// TerminalCtrlCHandlerPlugin

/// Setup of default task pools: [`AsyncComputeTaskPool`], [`ComputeTaskPool`], [`IoTaskPool`].
#[derive(Default)]
pub struct TaskPoolPlugin {
    /// Options for the [`TaskPool`](voker_task::TaskPool) created at application start.
    pub task_pool_options: TaskPoolOptions,
}

impl Plugin for TaskPoolPlugin {
    fn build(&self, app: &mut App) {
        // Setup the default voker task pools
        self.task_pool_options.create_default_pools();

        fn tick_global_task_pools(_: NonSendMarker) {
            let ticker = TaskPool::local_ticker();
            while ticker.try_tick() {}
        }

        app.add_systems(crate::Last, tick_global_task_pools);
    }
}

// -----------------------------------------------------------------------------
// TaskPoolThreadAssignmentPolicy

/// Helper for configuring and creating the default task pools. For end-users who want full control,
/// set up [`TaskPoolPlugin`]
#[derive(Clone, Debug)]
pub struct TaskPoolOptions {
    /// If the number of physical cores is less than `min_total_threads`, force using
    /// `min_total_threads`
    pub min_total_threads: usize,
    /// If the number of physical cores is greater than `max_total_threads`, force using
    /// `max_total_threads`
    pub max_total_threads: usize,
    /// Used to determine number of IO threads to allocate
    pub io: TaskPoolThreadAssignmentPolicy,
    /// Used to determine number of async compute threads to allocate
    pub async_compute: TaskPoolThreadAssignmentPolicy,
    /// Used to determine number of compute threads to allocate
    pub compute: TaskPoolThreadAssignmentPolicy,
}

/// Defines a simple way to determine how many threads to use given the number of remaining cores
/// and number of total cores
#[derive(Clone)]
pub struct TaskPoolThreadAssignmentPolicy {
    /// Force using at least this many threads
    pub min_threads: usize,
    /// Under no circumstance use more than this many threads for this pool
    pub max_threads: usize,
    /// Target using this percentage of total cores, clamped by `min_threads` and `max_threads`. It is
    /// permitted to use 1.0 to try to use all remaining threads
    pub percent: f32,
    /// Callback that is invoked once for every created thread as it starts.
    /// This configuration will be ignored under wasm platform.
    pub on_thread_spawn: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
    /// Callback that is invoked once for every created thread as it terminates
    /// This configuration will be ignored under wasm platform.
    pub on_thread_destroy: Option<Arc<dyn Fn() + Send + Sync + 'static>>,
}

impl Debug for TaskPoolThreadAssignmentPolicy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TaskPoolThreadAssignmentPolicy")
            .field("min_threads", &self.min_threads)
            .field("max_threads", &self.max_threads)
            .field("percent", &self.percent)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Policy Implementation

impl Default for TaskPoolOptions {
    fn default() -> Self {
        TaskPoolOptions {
            // By default, use however many cores are available on the system
            min_total_threads: 1,
            max_total_threads: usize::MAX,

            // Use 25% of cores for IO, at least 1, no more than 4
            io: TaskPoolThreadAssignmentPolicy {
                min_threads: 1,
                max_threads: 4,
                percent: 0.25,
                on_thread_spawn: None,
                on_thread_destroy: None,
            },

            // Use 25% of cores for async compute, at least 1, no more than 4
            async_compute: TaskPoolThreadAssignmentPolicy {
                min_threads: 1,
                max_threads: 4,
                percent: 0.25,
                on_thread_spawn: None,
                on_thread_destroy: None,
            },

            // Use all remaining cores for compute (at least 1)
            compute: TaskPoolThreadAssignmentPolicy {
                min_threads: 1,
                max_threads: usize::MAX,
                percent: 1.0, // This 1.0 here means "whatever is left over"
                on_thread_spawn: None,
                on_thread_destroy: None,
            },
        }
    }
}

impl TaskPoolThreadAssignmentPolicy {
    /// Determine the number of threads to use for this task pool
    fn get_thread_num(&self, remaining_threads: usize, total_threads: usize) -> usize {
        assert!(self.percent >= 0.0);
        let proportion = total_threads as f32 * self.percent;
        let mut desired = proportion as usize;

        // Equivalent to round() for positive floats without libm requirement for
        // no_std compatibility
        if proportion - desired as f32 >= 0.5 {
            desired += 1;
        }

        // Limit ourselves to the number of cores available
        desired = desired.min(remaining_threads);

        // Clamp by min_threads, max_threads. (This may result in us using more threads than are
        // available, this is intended. An example case where this might happen is a device with
        // <= 2 threads.
        desired.clamp(self.min_threads, self.max_threads)
    }
}

impl TaskPoolOptions {
    /// Create a configuration that forces using the given number of threads.
    pub fn with_thread_num(thread_count: usize) -> Self {
        TaskPoolOptions {
            min_total_threads: thread_count,
            max_total_threads: thread_count,
            ..Default::default()
        }
    }

    /// Inserts the default thread pools into the given resource map based on the configured values
    pub fn create_default_pools(&self) {
        let total_threads = voker_os::thread::available_parallelism()
            .get()
            .clamp(self.min_total_threads, self.max_total_threads);

        log::debug!("Assigning {total_threads} cores to default task pools");

        let mut remaining_threads = total_threads;

        {
            // Determine the number of IO threads we will use
            let io_threads = self.io.get_thread_num(remaining_threads, total_threads);

            log::debug!("IO Threads: {io_threads}");
            remaining_threads = remaining_threads.saturating_sub(io_threads);

            IoTaskPool::get_or_init(|| {
                let mut builder = TaskPoolBuilder::new()
                    .thread_num(io_threads)
                    .thread_name("IO Task Pool");

                if let Some(f) = self.io.on_thread_spawn.clone() {
                    builder = builder.on_thread_spawn(move || f());
                }

                if let Some(f) = self.io.on_thread_destroy.clone() {
                    builder = builder.on_thread_destroy(move || f());
                }

                builder.build()
            });
        }

        {
            // Determine the number of async compute threads we will use
            let async_compute_threads =
                self.async_compute.get_thread_num(remaining_threads, total_threads);

            log::debug!("Async Compute Threads: {async_compute_threads}");
            remaining_threads = remaining_threads.saturating_sub(async_compute_threads);

            AsyncComputeTaskPool::get_or_init(|| {
                let mut builder = TaskPoolBuilder::new()
                    .thread_num(async_compute_threads)
                    .thread_name("Async Compute Task Pool");

                if let Some(f) = self.async_compute.on_thread_spawn.clone() {
                    builder = builder.on_thread_spawn(move || f());
                }

                if let Some(f) = self.async_compute.on_thread_destroy.clone() {
                    builder = builder.on_thread_destroy(move || f());
                }

                builder.build()
            });
        }

        {
            // Determine the number of compute threads we will use
            // This is intentionally last so that an end user can specify 1.0 as the percent
            let compute_threads = self.compute.get_thread_num(remaining_threads, total_threads);

            log::debug!("Compute Threads: {compute_threads}");

            ComputeTaskPool::get_or_init(|| {
                let mut builder = TaskPoolBuilder::new()
                    .thread_num(compute_threads)
                    .thread_name("Compute Task Pool");

                if let Some(f) = self.compute.on_thread_spawn.clone() {
                    builder = builder.on_thread_spawn(move || f());
                }

                if let Some(f) = self.compute.on_thread_destroy.clone() {
                    builder = builder.on_thread_destroy(move || f());
                }

                builder.build()
            });
        }
    }
}
