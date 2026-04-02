mod multi;
mod single;

pub use multi::MultiThreadedExecutor;
pub use single::SingleThreadedExecutor;

// -----------------------------------------------------------------------------
// Exports

use super::SystemSchedule;
use crate::error::ErrorHandler;
use crate::world::World;

/// Runtime interface for executing a compiled system schedule.
///
/// Implementors are responsible for traversing dependency metadata in
/// [`SystemSchedule`] and invoking systems in a valid order while handling
/// errors through the provided [`ErrorHandler`].
pub trait SystemExecutor: Send + Sync {
    /// Returns the executor flavor.
    fn kind(&self) -> ExecutorKind;

    /// Initializes executor-internal state from a compiled schedule.
    ///
    /// Called when the schedule shape changes or when an executor is first used.
    fn init(&mut self, schedule: &SystemSchedule);

    /// Executes one schedule tick.
    ///
    /// Implementations should respect dependency ordering and may parallelize
    /// independent systems depending on [`ExecutorKind`].
    fn run(&mut self, schedule: &mut SystemSchedule, world: &mut World, handler: ErrorHandler);
}

/// Execution strategy used by a schedule.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExecutorKind {
    /// Always run systems on a single thread.
    #[cfg_attr(any(target_arch = "wasm32", not(feature = "std")), default)]
    SingleThreaded,
    /// Run independent systems in parallel on multiple threads.
    #[cfg_attr(all(not(target_arch = "wasm32"), feature = "std"), default)]
    MultiThreaded,
}

// -----------------------------------------------------------------------------
// MultiThreadExecutor

use crate::resource::Resource;
use voker_os::sync::Arc;
use voker_task::ThreadExecutor;

/// Handle to the main-thread task executor.
///
/// Stored as a resource to make main-thread execution facilities available
/// to ECS systems and scheduling utilities.
#[derive(Clone)]
pub struct MainThreadExecutor(pub Arc<ThreadExecutor<'static>>);

impl Resource for MainThreadExecutor {
    const MUTABLE: bool = false;
}
