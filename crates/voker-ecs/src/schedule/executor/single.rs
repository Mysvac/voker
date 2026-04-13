use alloc::vec::Vec;
use core::panic::AssertUnwindSafe;

use crate::error::{ErrorContext, GameError};
use crate::schedule::schedule::SystemScheduleView;
use crate::schedule::{ExecutorKind, SystemExecutor, SystemObject, SystemSchedule};
use crate::world::World;

/// Runs the schedule using a single thread.
///
/// Useful if you're dealing with a single-threaded environment,
/// saving your threads for other things, or just trying minimize overhead.
pub struct SingleThreadedExecutor {
    condition_incoming: Vec<u32>,
}

impl SingleThreadedExecutor {
    /// Creates a new single-threaded executor.
    pub const fn new() -> Self {
        Self {
            condition_incoming: Vec::new(),
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

    fn init(&mut self, schedule: &SystemSchedule) {
        let keys = schedule.keys();
        let systems = schedule.systems();
        assert_eq!(keys.len(), systems.len());
    }

    /// Runs all systems sequentially on the current thread.
    fn run(
        &mut self,
        schedule: &mut SystemSchedule,
        world: &mut World,
        handler: fn(GameError, ErrorContext),
    ) {
        let SystemScheduleView {
            keys,
            systems,
            condition_incoming,
            condition_outgoing,
            ..
        } = schedule.view();

        let system_count = keys.len();
        assert_eq!(system_count, systems.len());
        assert_eq!(system_count, condition_incoming.len());
        assert_eq!(system_count, condition_outgoing.len());

        self.condition_incoming.clear();
        self.condition_incoming.extend_from_slice(condition_incoming);

        systems.iter_mut().enumerate().for_each(|(index, obj)| {
            if self.condition_incoming[index] != 0 {
                return; // next system
            }

            match obj {
                SystemObject::Action { system, .. } => {
                    let func = AssertUnwindSafe(|| unsafe {
                        system.run_raw((), world.unsafe_world()).unwrap_or_else(|e| {
                            voker_utils::cold_path();
                            let last_run = system.last_run();
                            let name = system.id().name();
                            let ctx = ErrorContext::System { name, last_run };
                            handler(e.into(), ctx);
                        })
                    });

                    #[cfg(feature = "std")]
                    if let Err(payload) = ::std::panic::catch_unwind(func) {
                        voker_utils::cold_path();
                        log::error!("Encountered a panic in system `{}`!", system.id());
                        ::std::eprintln!("Encountered a panic in system `{}`!", system.id());
                        ::std::panic::resume_unwind(payload);
                    }

                    #[cfg(not(feature = "std"))]
                    (func)();

                    if system.is_deferred() {
                        system.apply_deferred(unsafe { world.unsafe_world().full_mut() });
                    }

                    condition_outgoing[index].iter().for_each(|&to| {
                        self.condition_incoming[to as usize] -= 1;
                    });
                }
                SystemObject::Condition { system, .. } => {
                    let func = AssertUnwindSafe(|| unsafe {
                        system.run_raw((), world.unsafe_world()).unwrap_or_else(|e| {
                            voker_utils::cold_path();
                            let last_run = system.last_run();
                            let name = system.id().name();
                            let ctx = ErrorContext::System { name, last_run };
                            handler(e.into(), ctx);
                            false
                        })
                    });

                    #[cfg(feature = "std")]
                    let condition = ::std::panic::catch_unwind(func).unwrap_or_else(|payload| {
                        voker_utils::cold_path();
                        log::error!("Encountered a panic in system `{}`!", system.id());
                        ::std::eprintln!("Encountered a panic in system `{}`!", system.id());
                        ::std::panic::resume_unwind(payload);
                    });

                    #[cfg(not(feature = "std"))]
                    let condition = (func)();

                    if system.is_deferred() {
                        system.apply_deferred(unsafe { world.unsafe_world().full_mut() });
                    }

                    if condition {
                        condition_outgoing[index].iter().for_each(|&to| {
                            self.condition_incoming[to as usize] -= 1;
                        });
                    }
                }
            }
        });

        world.flush();
    }
}
