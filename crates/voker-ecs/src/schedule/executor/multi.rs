use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::any::Any;
use core::panic::AssertUnwindSafe;

use voker_os::sync::{Mutex, PoisonError, SyncUnsafeCell};
use voker_os::utils::SegQueue;
use voker_task::{ComputeTaskPool, Scope, TaskPool};
use voker_utils::hash::SparseHashSet;

use super::{MainThreadExecutor, SystemExecutor};

use crate::error::{ErrorContext, GameError};
use crate::schedule::schedule::{ConflictTable, SystemScheduleView};
use crate::schedule::{ExecutorKind, SystemObject, SystemSchedule};
use crate::system::{SystemFlags, SystemId};
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// State

struct ExecutorState {
    incoming: Vec<u32>,
    condition_incoming: Vec<u32>,
    ready_systems: VecDeque<u32>,
    running_systems: SparseHashSet<u32>,
    deferred_systems: Vec<u32>,
}

struct CompletedSignal {
    index: u32,
    deferred: bool,
    meet_condition: bool,
}

/// Runs the schedule on multiple worker threads.
///
/// The executor tracks dependency counters (`incoming`) and a ready queue,
/// spawning tasks for systems whose dependencies are satisfied.
///
/// Non-send systems are dispatched to the external/main-thread executor when
/// available; sendable systems run on the compute task pool.
pub struct MultiThreadedExecutor {
    state: Mutex<ExecutorState>,
    completed: SegQueue<CompletedSignal>,
    panic_payload: Mutex<Option<Box<dyn Any + Send>>>,
}

#[derive(Copy, Clone)]
struct Context<'scope, 'env, 'sys> {
    world: UnsafeWorld<'env>,
    executor: &'env MultiThreadedExecutor,
    scope: &'scope Scope<'scope, 'env, ()>,
    systems: &'sys [SyncUnsafeCell<SystemObject>],
    outgoing: &'sys [&'sys [u32]],
    condition_outgoing: &'sys [&'sys [u32]],
    conflict_table: &'sys ConflictTable,
    error_handler: fn(GameError, ErrorContext),
}

// -----------------------------------------------------------------------------
// ExecutorState Implementation

impl ExecutorState {
    const fn new() -> Self {
        Self {
            incoming: Vec::new(),
            condition_incoming: Vec::new(),
            ready_systems: VecDeque::new(),
            running_systems: SparseHashSet::new(),
            deferred_systems: Vec::new(),
        }
    }

    fn init(&mut self, schedule: &SystemSchedule) {
        let systen_count = schedule.keys().len();
        let full_size_hint = systen_count + (systen_count >> 3);
        let half_size_hint = systen_count + (systen_count >> 2);

        self.incoming = Vec::with_capacity(full_size_hint);
        self.condition_incoming = Vec::with_capacity(full_size_hint);
        self.running_systems = SparseHashSet::with_capacity(half_size_hint);
        self.ready_systems = VecDeque::with_capacity(half_size_hint);
        self.deferred_systems = Vec::with_capacity(half_size_hint);
    }

    fn reset(&mut self, schedule: &SystemSchedule) {
        let system_count = schedule.keys().len();
        assert_eq!(system_count, schedule.systems().len());
        assert_eq!(system_count, schedule.incoming().len());
        assert_eq!(system_count, schedule.outgoing().len());
        assert_eq!(system_count, schedule.condition_incoming().len());
        assert_eq!(system_count, schedule.condition_outgoing().len());

        self.incoming.clear();
        self.condition_incoming.clear();
        self.ready_systems.clear();
        self.running_systems.clear();
        self.deferred_systems.clear();

        self.incoming.extend_from_slice(schedule.incoming());
        self.condition_incoming
            .extend_from_slice(schedule.condition_incoming());

        self.incoming.iter().enumerate().for_each(|(idx, &num)| {
            if num == 0 {
                // `incoming` logically includes `condition_incoming`, therefore,
                // nodes with an initial 0 incoming do not need to check the condition.
                self.ready_systems.push_back(idx as u32);
            }
        });
    }
}

// -----------------------------------------------------------------------------
// MultiThreadedExecutor New

impl Default for MultiThreadedExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiThreadedExecutor {
    /// Creates a new multi-threaded executor.
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(ExecutorState::new()),
            completed: SegQueue::new(),
            panic_payload: Mutex::new(None),
        }
    }
}

// -----------------------------------------------------------------------------
// Context Implementation

impl<'scope, 'env: 'scope, 'sys: 'scope> Context<'scope, 'env, 'sys> {
    fn new(
        world: &'env mut World,
        executor: &'env MultiThreadedExecutor,
        schedule: &'sys mut SystemSchedule,
        scope: &'scope Scope<'scope, 'env, ()>,
        error_handler: fn(GameError, ErrorContext),
    ) -> Self {
        let SystemScheduleView {
            systems,
            outgoing,
            condition_outgoing,
            conflict,
            ..
        } = schedule.view();

        Self {
            world: world.unsafe_world(),
            executor,
            scope,
            systems: SyncUnsafeCell::from_mut(systems).transpose(),
            outgoing,
            condition_outgoing,
            conflict_table: conflict,
            error_handler,
        }
    }

    fn push_completed_system(
        &self,
        ident: SystemId,
        system_index: u32,
        deferred: bool,
        result: Result<bool, Box<dyn Any + Send>>,
    ) {
        let meet = result.unwrap_or_else(|payload| {
            voker_utils::cold_path();
            log::error!("Encountered a panic in system `{}`!", ident);
            #[cfg(feature = "std")]
            ::std::eprintln!("Encountered a panic in system `{}`!", ident);
            *self.executor.panic_payload.lock().unwrap() = Some(payload);
            false
        });

        let signal = CompletedSignal {
            index: system_index,
            meet_condition: meet,
            deferred,
        };

        self.executor.completed.push(signal);

        self.tick();
    }

    fn handle_completed_system(&self, state: &mut ExecutorState, signal: CompletedSignal) {
        let index = signal.index;
        let deferred = signal.deferred;
        let meet_condition = signal.meet_condition;

        if deferred {
            state.deferred_systems.push(index);
        }

        if meet_condition {
            self.condition_outgoing[index as usize].iter().for_each(|&to| {
                state.condition_incoming[to as usize] -= 1;
            });
        }

        self.outgoing[index as usize].iter().for_each(|&to| {
            let target = &mut state.incoming[to as usize];
            *target -= 1;

            if *target == 0 {
                if state.condition_incoming[to as usize] == 0 {
                    // push_front: Prioritize new tasks to avoid repetitive checks.
                    state.ready_systems.push_front(to);
                } else {
                    // The target system can not be executed.
                    // But we need to allow its subsequent system to run.
                    let signal = CompletedSignal {
                        index: to,
                        meet_condition: false,
                        deferred: false,
                    };
                    self.handle_completed_system(state, signal);
                }
            }
        });
    }

    fn spawn_ready_tasks(&self, state: &mut ExecutorState) {
        let len = state.ready_systems.len();
        for _ in 0..len {
            let Some(index) = state.ready_systems.pop_front() else {
                return;
            };

            let is_conflict = state
                .running_systems
                .iter()
                .any(|&running| unsafe { self.conflict_table.is_conflict(index, running) });

            if !is_conflict {
                self.spawn_system_task(state, index);
            } else {
                state.ready_systems.push_back(index);

                if !self.executor.completed.is_empty() {
                    return; // Prioritize handle new signale to reduce access conflicts.
                }
            }
        }
    }

    fn spawn_system_task(&self, state: &mut ExecutorState, index: u32) {
        let obj = unsafe { &mut *self.systems[index as usize].get() };

        let ident = obj.id();
        let flags = obj.flags();
        // call `flags + contains` is faster then `System::is_xxx`
        let deferred = flags.contains(SystemFlags::DEFERRED);
        let non_send = flags.contains(SystemFlags::NON_SEND);
        let exclusive = flags.contains(SystemFlags::EXCLUSIVE);

        let need_apply_deferred = exclusive && !state.deferred_systems.is_empty();

        let apply_deferred: Option<Box<dyn FnOnce() + Send>> = if need_apply_deferred {
            let world = self.world;
            let systems = self.systems;
            let panic_payload = &self.executor.panic_payload;
            // ↓ We do not take deferred_systems, avoid memory reallocation.
            let mut deferred: Vec<u32> = Vec::new();
            deferred.append(&mut state.deferred_systems);

            Some(Box::new(move || {
                let world = unsafe { world.full_mut() };
                for index in deferred {
                    let func = AssertUnwindSafe(|| {
                        let system = unsafe { &mut *systems[index as usize].get() };
                        system.defer(world.deferred());
                        system.apply_deferred(world);
                        world.flush();
                    });

                    #[cfg(feature = "std")]
                    if let Err(e) = ::std::panic::catch_unwind(func) {
                        voker_utils::cold_path();
                        *panic_payload.lock().unwrap() = Some(e);
                    }

                    #[cfg(not(feature = "std"))]
                    (func)();
                }
            }))
        } else {
            None
        };

        match obj {
            SystemObject::Action { system, .. } => {
                let context: Context<'scope, 'env, 'sys> = *self;

                let task = async move {
                    if let Some(apply_deferred) = apply_deferred {
                        apply_deferred();
                    }

                    let func = AssertUnwindSafe(|| unsafe {
                        if let Err(e) = system.run((), context.world) {
                            voker_utils::cold_path();
                            let last_run = system.get_last_run();
                            let name = system.id().name();
                            let ctx = ErrorContext::System { name, last_run };
                            (context.error_handler)(e, ctx);
                            return false; // Error -> false
                        }
                        true // Success -> true
                    });

                    #[cfg(feature = "std")]
                    let result = ::std::panic::catch_unwind(func);

                    #[cfg(not(feature = "std"))]
                    let result = Ok((func)());

                    context.push_completed_system(ident, index, deferred, result);
                };

                if non_send {
                    voker_utils::cold_path();
                    self.scope.spawn_remote(task);
                } else {
                    self.scope.spawn(task);
                }
            }
            SystemObject::Condition { system, .. } => {
                let context: Context<'scope, 'env, 'sys> = *self;

                let task = async move {
                    if let Some(apply_deferred) = apply_deferred {
                        apply_deferred();
                    }

                    let func = AssertUnwindSafe(|| unsafe {
                        system.run((), context.world).unwrap_or_else(|e| {
                            voker_utils::cold_path();
                            let last_run = system.get_last_run();
                            let name = system.id().name();
                            let ctx = ErrorContext::System { name, last_run };
                            (context.error_handler)(e, ctx);
                            false // Error -> false
                        })
                    });

                    #[cfg(feature = "std")]
                    let result = ::std::panic::catch_unwind(func);

                    #[cfg(not(feature = "std"))]
                    let result = Ok((func)());

                    context.push_completed_system(ident, index, deferred, result);
                };

                if non_send {
                    voker_utils::cold_path();
                    self.scope.spawn_remote(task);
                } else {
                    self.scope.spawn(task);
                }
            }
        }
    }

    fn tick_internal(&self, state: &mut ExecutorState) {
        let completed_queue = &self.executor.completed;

        while let Some(signal) = completed_queue.pop() {
            self.handle_completed_system(state, signal);
        }

        self.spawn_ready_tasks(state);
    }

    fn tick(&self) {
        loop {
            let Ok(mut guard) = self.executor.state.try_lock() else {
                // try_lock failed, there are already other threads doing this.
                return;
            };
            self.tick_internal(&mut guard);
            // Make sure we drop the guard before checking
            // completed.is_empty(), or we could lose events.
            drop(guard);
            // We cannot check `is_empty` before `tick_internal`
            // because the initial tasks without dependencies are
            // in a ready state and not in the queue.
            if self.executor.completed.is_empty() {
                return;
            }
        }
    }
}

// -----------------------------------------------------------------------------
// SystemExecutor Implementation

impl SystemExecutor for MultiThreadedExecutor {
    /// Returns [`ExecutorKind::MultiThreaded`].
    fn kind(&self) -> ExecutorKind {
        ExecutorKind::MultiThreaded
    }

    /// Initializes internal scheduling buffers from a compiled schedule.
    ///
    /// This pre-allocates storage for dependency counters and ready queues.
    fn init(&mut self, schedule: &SystemSchedule) {
        self.state
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .init(schedule);
    }

    /// Executes the schedule using task-based parallel dispatch.
    ///
    /// Systems are launched when all incoming dependencies are resolved.
    /// Reported system errors are forwarded to `handler`.
    ///
    /// If any task panics, the panic payload is captured and rethrown after the
    /// task scope completes.
    fn run(
        &mut self,
        schedule: &mut SystemSchedule,
        world: &mut World,
        handler: fn(GameError, ErrorContext),
    ) {
        if schedule.keys().is_empty() {
            return;
        }

        self.state
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .reset(schedule);

        self.state.clear_poison(); // optional

        let main_thread_ex = world.get_resource::<MainThreadExecutor>().map(|e| e.0.clone());
        let remote_ex = main_thread_ex.as_deref();

        let task_pool = ComputeTaskPool::get_or_init(TaskPool::default);
        task_pool.scope_with(false, remote_ex, |scope| {
            let context = Context::new(world, self, schedule, scope, handler);
            context.tick();
        });

        self.state
            .get_mut()
            .unwrap()
            .deferred_systems
            .iter()
            .for_each(|&index| {
                let func = AssertUnwindSafe(|| {
                    let system = &mut schedule.systems_mut()[index as usize];
                    system.defer(world.deferred());
                    system.apply_deferred(world);
                });

                #[cfg(feature = "std")]
                if let Err(e) = ::std::panic::catch_unwind(func) {
                    *self.panic_payload.get_mut().unwrap() = Some(e);
                }

                #[cfg(not(feature = "std"))]
                (func)();
            });

        // check to see if there was a panic
        let payload = self.panic_payload.get_mut().unwrap().take();

        world.flush();

        #[cfg(feature = "std")]
        if let Some(payload) = payload {
            ::std::panic::resume_unwind(payload);
        }

        #[cfg(not(feature = "std"))]
        assert!(payload.is_none());
    }
}
