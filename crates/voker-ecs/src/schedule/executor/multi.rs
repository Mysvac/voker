use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::any::Any;
use core::panic::AssertUnwindSafe;
use fixedbitset::FixedBitSet;

use voker_os::sync::{Mutex, PoisonError, SyncUnsafeCell};
use voker_os::utils::{SegQueue, SpinLock};
use voker_task::{ComputeTaskPool, Scope, TaskPool};
use voker_utils::vec::FastVec;

use super::{MainThreadExecutor, SystemExecutor};

use crate::error::{ErrorContext, GameError};
use crate::schedule::schedule::{ConflictTable, SystemScheduleView};
use crate::schedule::{ExecutorKind, SystemObject, SystemSchedule};
use crate::system::{SystemFlags, SystemId};
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// State

/// Mutable scheduling state reused between runs.
///
/// This stores runtime counters and queues derived from `SystemSchedule`.
/// Buffers are pre-allocated in `init` and refreshed in `reset`.
struct ExecutorState {
    /// Remaining dependency counts for each system.
    incoming: Vec<u32>,
    /// Remaining condition dependencies for each system.
    condition_incoming: Vec<u32>,
    /// Runnable systems whose dependencies are currently satisfied.
    ready_systems: VecDeque<u32>,
    /// Systems currently executing.
    running_systems: FixedBitSet,
    /// Deferred systems that have run and still need `apply_deferred`.
    deferred_systems: Vec<u32>,
}

/// Completion event emitted by system tasks.
///
/// `meet_condition` is used both for real condition systems and for normal
/// systems, where `true` means execution succeeded.
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
    panic_payload: SpinLock<Option<Box<dyn Any + Send>>>,
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
            running_systems: FixedBitSet::new(),
            deferred_systems: Vec::new(),
        }
    }

    fn init(&mut self, schedule: &SystemSchedule) {
        let system_count = schedule.keys().len();
        let full_size_hint = system_count + (system_count >> 3);
        let half_size_hint = system_count >> 3;

        self.incoming = Vec::with_capacity(full_size_hint);
        self.condition_incoming = Vec::with_capacity(full_size_hint);
        self.running_systems = FixedBitSet::with_capacity(full_size_hint);
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

        self.running_systems.grow(system_count);
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
            panic_payload: SpinLock::new(None),
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

    /// Pushes a completion signal and opportunistically advances the scheduler.
    fn push_completed_system(
        &self,
        ident: SystemId,
        system_index: u32,
        mut deferred: bool,
        result: Result<bool, Box<dyn Any + Send>>,
    ) {
        let meet = result.unwrap_or_else(|payload| {
            voker_utils::cold_path();
            log::error!("Encountered a panic in system `{}`!", ident);
            #[cfg(feature = "std")]
            ::std::eprintln!("Encountered a panic in system `{}`!", ident);
            *self.executor.panic_payload.lock() = Some(payload);
            deferred = false;
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

    /// Consumes completion events and propagates dependency updates.
    fn handle_completed_system(&self, state: &mut ExecutorState, signal: CompletedSignal) {
        // Use an explicit stack to avoid deep recursion on long skip chains.
        let mut buffer: FastVec<CompletedSignal, 5> = FastVec::new();

        let pending = buffer.data();
        // SAFETY: Inlined capacity > 0.
        unsafe { pending.push_unchecked(signal) };

        while let Some(signal) = pending.pop() {
            let index = signal.index;
            let deferred = signal.deferred;
            let meet_condition = signal.meet_condition;

            // SAFETY: `ExecutorState::reset` grows `running_systems` to
            // `system_count`, and schedule indices are in range.
            unsafe {
                state.running_systems.remove_unchecked(index as usize);
            }

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
                        // push_front: prioritize newly unblocked tasks.
                        state.ready_systems.push_front(to);
                    } else {
                        // Skip systems whose conditions are unresolved/failed,
                        // but continue propagating completion to dependents.
                        pending.push(CompletedSignal {
                            index: to,
                            meet_condition: false,
                            deferred: false,
                        });
                    }
                }
            });
        }
    }

    /// Resolves ready no-op systems immediately and propagates their completion.
    fn handle_no_op_systems(&self, state: &mut ExecutorState) {
        // Collect indices first, then remove from back to front to keep them valid.
        let mut buffer: FastVec<usize, 5> = FastVec::new();
        let no_op_systems = buffer.data();

        for (index, &id) in state.ready_systems.iter().enumerate() {
            let obj = unsafe { &*self.systems[id as usize].get() };
            if obj.is_no_op() {
                no_op_systems.push(index);
            }
        }

        while let Some(back) = no_op_systems.pop() {
            let signal = CompletedSignal {
                index: state.ready_systems.swap_remove_back(back).unwrap(),
                meet_condition: true,
                deferred: false,
            };
            self.handle_completed_system(state, signal);
        }
    }

    fn handle_deferred_systems(
        &self,
        state: &mut ExecutorState,
    ) -> Box<dyn FnOnce() + Send + 'scope> {
        let world = self.world;
        let systems = self.systems;
        let panic_payload = &self.executor.panic_payload;

        // Drain without reallocating by reusing the existing buffer capacity.
        let mut deferred: Vec<u32> = Vec::new();
        deferred.append(&mut state.deferred_systems);

        Box::new(move || {
            let world = unsafe { world.full_mut() };
            world.flush();

            for index in deferred {
                let func = AssertUnwindSafe(|| {
                    let system = unsafe { &mut *systems[index as usize].get() };
                    system.apply_deferred(world);
                });

                #[cfg(feature = "std")]
                if let Err(e) = ::std::panic::catch_unwind(func) {
                    voker_utils::cold_path();
                    *panic_payload.lock() = Some(e);
                }

                #[cfg(not(feature = "std"))]
                (func)();
            }

            world.flush();
        })
    }

    /// Tries to spawn all currently ready systems that do not conflict.
    fn spawn_ready_tasks(&self, state: &mut ExecutorState) {
        let len = state.ready_systems.len();
        for _ in 0..len {
            let Some(index) = state.ready_systems.pop_front() else {
                return;
            };

            let is_conflict = state
                .running_systems
                .ones()
                .any(|running| unsafe { self.conflict_table.is_conflict(index, running as u32) });

            if !is_conflict {
                self.spawn_system_task(state, index);
            } else {
                state.ready_systems.push_back(index);

                if !self.executor.completed.is_empty() {
                    return; // Prioritize handling fresh completion signals.
                }
            }
        }
    }

    /// Spawns one runnable system task and updates running/deferred bookkeeping.
    fn spawn_system_task(&self, state: &mut ExecutorState, index: u32) {
        let obj = unsafe { &mut *self.systems[index as usize].get() };

        let ident = obj.id();
        let flags = obj.flags();
        // Reading raw flags avoids repeated virtual method calls.
        let deferred = flags.contains(SystemFlags::DEFERRED);
        let non_send = flags.contains(SystemFlags::NON_SEND);
        let exclusive = flags.contains(SystemFlags::EXCLUSIVE);

        let need_apply_deferred = exclusive && !state.deferred_systems.is_empty();

        let apply_deferred: Option<Box<dyn FnOnce() + Send>> = if need_apply_deferred {
            Some(self.handle_deferred_systems(state))
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
                        if let Err(e) = system.run_raw((), context.world) {
                            voker_utils::cold_path();
                            let last_run = system.last_run();
                            let name = system.id().name();
                            let ctx = ErrorContext::System { name, last_run };
                            (context.error_handler)(e.into(), ctx);
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

                // SAFETY: `ExecutorState::reset` grows `running_systems` to
                // `system_count`, and schedule indices are in range.
                unsafe {
                    state.running_systems.insert_unchecked(index as usize);
                }

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
                        system.run_raw((), context.world).unwrap_or_else(|e| {
                            voker_utils::cold_path();
                            let last_run = system.last_run();
                            let name = system.id().name();
                            let ctx = ErrorContext::System { name, last_run };
                            (context.error_handler)(e.into(), ctx);
                            false // Error -> false
                        })
                    });

                    #[cfg(feature = "std")]
                    let result = ::std::panic::catch_unwind(func);

                    #[cfg(not(feature = "std"))]
                    let result = Ok((func)());

                    context.push_completed_system(ident, index, deferred, result);
                };

                // SAFETY: `ExecutorState::reset` grows `running_systems` to
                // `system_count`, and schedule indices are in range.
                unsafe {
                    state.running_systems.insert_unchecked(index as usize);
                }

                if non_send {
                    voker_utils::cold_path();
                    self.scope.spawn_remote(task);
                } else {
                    self.scope.spawn(task);
                }
            }
        }

        // Handle no-op systems after spawning to keep worker execution flowing.
        if exclusive && !state.ready_systems.is_empty() {
            self.handle_no_op_systems(state);
        }
    }

    /// Drains completion events, then schedules newly-unblocked tasks.
    fn tick_internal(&self, state: &mut ExecutorState) {
        let completed_queue = &self.executor.completed;

        while let Some(signal) = completed_queue.pop() {
            self.handle_completed_system(state, signal);
        }

        self.spawn_ready_tasks(state);
    }

    /// Progresses scheduling work until no fresh completion event is observed.
    fn tick(&self) {
        loop {
            let Ok(mut guard) = self.executor.state.try_lock() else {
                // Another thread is already advancing scheduling state.
                return;
            };
            self.tick_internal(&mut guard);
            // Make sure we drop the guard before checking
            // completed.is_empty(), or we could lose events.
            drop(guard);
            // We cannot check `is_empty` before `tick_internal` because
            // initial dependency-free systems start in `ready_systems`,
            // not in `completed`.
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
    /// Systems are launched when all incoming dependencies are resolved and
    /// access-conflict checks pass.
    ///
    /// Deferred systems are tracked during execution and applied at sync points:
    /// - before spawning an exclusive system when needed,
    /// - and once after the worker scope drains.
    ///
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

        // The executor handles panics explicitly; clear stale poison state.
        self.state.clear_poison();

        let main_thread_ex = world.get_resource::<MainThreadExecutor>().map(|e| e.0.clone());
        let remote_ex = main_thread_ex.as_deref();

        let task_pool = ComputeTaskPool::get_or_init(TaskPool::default);
        task_pool.scope_with(false, remote_ex, |scope| {
            let context = Context::new(world, self, schedule, scope, handler);
            context.tick();
        });

        self.state
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .deferred_systems
            .iter()
            .for_each(|&index| {
                let func = AssertUnwindSafe(|| {
                    schedule.systems_mut()[index as usize].apply_deferred(world);
                });

                #[cfg(feature = "std")]
                if let Err(e) = ::std::panic::catch_unwind(func) {
                    *self.panic_payload.get_mut() = Some(e);
                }

                #[cfg(not(feature = "std"))]
                (func)();
            });

        // Re-throw captured panic after scheduler cleanup.
        let payload = self.panic_payload.get_mut().take();

        world.flush();

        #[cfg(feature = "std")]
        if let Some(payload) = payload {
            ::std::panic::resume_unwind(payload);
        }

        #[cfg(not(feature = "std"))]
        assert!(payload.is_none());
    }
}
