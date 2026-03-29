use core::marker::PhantomData;
use core::panic::AssertUnwindSafe;

use crate::cfg;
use crate::error::{EcsError, ErrorContext};
use crate::schedule::schedule::SystemScheduleView;
use crate::schedule::{ExecutorKind, SystemExecutor, SystemSchedule};
use crate::world::World;

/// Runs the schedule using a single thread.
///
/// Useful if you're dealing with a single-threaded environment,
/// saving your threads for other things, or just trying minimize overhead.
pub struct SingleThreadedExecutor {
    // Sealed, direct creation is prohibited
    _marker: PhantomData<()>,
}

impl SingleThreadedExecutor {
    /// Creates a new single-threaded executor.
    ///
    /// Use this when deterministic, in-order execution on one thread is
    /// preferred over parallel throughput.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

impl Default for SingleThreadedExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemExecutor for SingleThreadedExecutor {
    /// Returns [`ExecutorKind::SingleThreaded`].
    fn kind(&self) -> ExecutorKind {
        ExecutorKind::SingleThreaded
    }

    /// Validates schedule shape before execution.
    ///
    /// The single-threaded executor expects `keys` and `systems` to have the
    /// same length and index alignment.
    fn init(&mut self, schedule: &SystemSchedule) {
        let keys = schedule.keys();
        let systems = schedule.systems();
        assert_eq!(keys.len(), systems.len());
    }

    /// Runs all systems sequentially on the current thread.
    ///
    /// Execution order follows `schedule.systems` order. System-returned errors
    /// are forwarded to `handler` with [`ErrorContext::System`].
    ///
    /// When `std` is available, each system call is wrapped in `catch_unwind`:
    /// panic information is printed and then rethrown.
    fn run(
        &mut self,
        schedule: &mut SystemSchedule,
        world: &mut World,
        handler: fn(EcsError, ErrorContext),
    ) {
        let SystemScheduleView {
            keys,
            systems,
            incoming,
            outgoing,
        } = schedule.view();
        let system_count = keys.len();
        assert_eq!(system_count, systems.len());
        assert_eq!(system_count, incoming.len());
        assert_eq!(system_count, outgoing.len());

        systems.iter_mut().for_each(|obj| {
            let system = &mut obj.system;
            let func = AssertUnwindSafe(|| unsafe {
                if let Err(e) = system.run((), world.unsafe_world()) {
                    let last_run = system.get_last_run();
                    let id = system.id();
                    let ctx = ErrorContext::System { id, last_run };
                    handler(e, ctx);
                }
            });

            cfg::std! {
                if {
                    if let Err(payload) = ::std::panic::catch_unwind(func) {
                        ::std::eprintln!("Encountered a panic in system `{}`!", system.id());
                        ::std::panic::resume_unwind(payload);
                    }
                } else {
                    (func)();
                }
            }
        });
    }
}
