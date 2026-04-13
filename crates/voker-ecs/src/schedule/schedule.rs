#![expect(clippy::module_inception, reason = "For better structure.")]

use core::fmt::Debug;

use alloc::boxed::Box;
use alloc::vec::Vec;

use fixedbitset::FixedBitSet;
use slotmap::{SecondaryMap, SlotMap};
use voker_utils::extra::PagePool;
use voker_utils::hash::{HashMap, HashSet, NoOpHashMap, NoOpHashSet, SparseHashMap};

use super::{ActionSystem, ConditionSystem, SystemKey, SystemObject};
use super::{Dag, InternedScheduleLabel, ScheduleLabel, SystemExecutor};
use super::{ExecutorKind, MultiThreadedExecutor, SingleThreadedExecutor};
use crate::schedule::{AnonymousSchedule, ToposortError};
use crate::schedule::{IntoSystemConfig, SystemConfig, SystemNode};
use crate::system::{IntoSystem, SystemId};
use crate::utils::DebugCheckedUnwrap;
use crate::world::World;

// -----------------------------------------------------------------------------
// Schedule

/// Execution graph for ECS systems.
///
/// # Two System Kinds
///
/// `Schedule` stores two kinds of systems: [`ConditionSystem`] and
/// [`ActionSystem`].
///
/// The only semantic difference is their output:
/// - `ConditionSystem` returns `bool`
/// - `ActionSystem` returns `()`
///
/// In practice, `ConditionSystem` can still run complex logic just like
/// `ActionSystem`.
///
/// # What Determines Execution Order
///
/// System execution is constrained by three factors:
/// - access conflicts
/// - explicit dependencies
/// - run conditions
///
/// ## Access Conflicts
///
/// Every system carries an access table. The executor enforces compatibility at
/// runtime.
///
/// In single-threaded mode, systems are not run concurrently, so cross-system
/// conflict checks are effectively free.
///
/// In multi-threaded mode, the executor checks whether `Ready` systems conflict
/// with currently `Running` systems. Only non-conflicting systems can start.
///
/// ## Explicit Dependencies And Run Conditions
///
/// Explicit dependencies are user-defined ordering constraints between systems
/// to guarantee visibility and sequencing.
///
/// Internally, explicit dependencies are maintained as a DAG.
///
/// Run conditions are also represented as a DAG. They introduce implicit
/// dependencies, so both DAGs are merged before topological scheduling.
///
/// ## Runtime States
///
/// A system can be observed in these states:
///
/// - `Waiting`: previous dependencies (explicit or implicit) are incomplete
/// - `Ready`: dependencies are complete
///   - `ReadyFalse`: run conditions evaluated to false
///   - `ReadyTrue`: run conditions evaluated to true
/// - `Running`: currently executing
/// - `Completed`: execution finished
///   - `CompletedFalse`: finished with false condition output
///   - `CompletedTrue`: finished with true condition output
///
/// Because implicit condition edges are included in dependency resolution,
/// systems entering `Ready` have already had their condition dependencies
/// evaluated.
///
/// If all conditions pass (and no access conflict exists), the system runs.
/// Otherwise, it is treated as completed with a false condition result.
///
/// This implies that systems that do not run (both condition and action kinds)
/// produce a false condition outcome, while successfully run action systems
/// produce true. In that sense, action systems can also be interpreted as
/// condition producers based on whether they actually executed.
pub struct Schedule {
    label: InternedScheduleLabel,
    allocator: Allocator,
    buffer: SystemBuffer,
    ordering: OrderingGraph,
    conflict: ConflictGraph,
    schedule: SystemSchedule,
    set_anchors: NoOpHashSet<SystemId>,
    executor: Box<dyn SystemExecutor>,
    executor_initialized: bool,
    is_changed: bool,
}

// -----------------------------------------------------------------------------
// Allocator

#[derive(Default)]
struct Allocator {
    slots: SlotMap<SystemKey, SystemId>,
    idents: NoOpHashMap<SystemId, SystemKey>,
}

// -----------------------------------------------------------------------------
// SystemBuffer

#[derive(Default)]
struct SystemBuffer {
    nodes: SecondaryMap<SystemKey, Option<SystemObject>>,
    uninit: Vec<SystemKey>,
}

// -----------------------------------------------------------------------------
// OrderingGraph

#[derive(Default)]
struct OrderingGraph {
    dependency: Dag<SystemKey>,
    condition: Dag<SystemKey>,
}

// -----------------------------------------------------------------------------
// ConflictTable

#[derive(Default)]
struct ConflictGraph {
    exclusive: HashSet<SystemKey>,
    conflicts: HashMap<SystemKey, HashSet<SystemKey>>,
}

#[derive(Default)]
pub struct ConflictTable {
    // We use complete matrices instead of triangles.
    // This has better cache affinity during traversal.
    lines: usize,
    table: FixedBitSet,
}

// -----------------------------------------------------------------------------
// SystemSchedule

/// Compiled schedule data consumed by executors.
///
/// This is a dense runtime representation derived from `Schedule` internals.
/// `keys` and `systems` share the same index. `incoming` stores dependency
/// counts, and `outgoing` stores adjacency lists by index.
#[derive(Default)]
pub struct SystemSchedule {
    keys: Vec<SystemKey>,
    systems: Vec<SystemObject>,
    conflict: ConflictTable,
    incoming: Vec<u32>,
    outgoing: Vec<&'static [u32]>,
    condition_incoming: Vec<u32>,
    condition_outgoing: Vec<&'static [u32]>,
    pool: PagePool<2048>,
}

unsafe impl Sync for SystemSchedule {}
unsafe impl Send for SystemSchedule {}

/// View of `SystemSchedule` for encapsulating internal implementation
pub struct SystemScheduleView<'s> {
    pub keys: &'s [SystemKey],
    pub systems: &'s mut [SystemObject],
    pub conflict: &'s ConflictTable,
    pub incoming: &'s [u32],
    pub outgoing: &'s [&'s [u32]],
    pub condition_incoming: &'s [u32],
    pub condition_outgoing: &'s [&'s [u32]],
}

impl SystemSchedule {
    pub fn view(&mut self) -> SystemScheduleView<'_> {
        let SystemSchedule {
            keys,
            systems,
            conflict,
            incoming,
            outgoing,
            condition_incoming,
            condition_outgoing,
            ..
        } = self;

        SystemScheduleView {
            keys,
            systems,
            conflict,
            incoming,
            outgoing,
            condition_incoming,
            condition_outgoing,
        }
    }

    pub fn systems_mut(&mut self) -> &mut [SystemObject] {
        &mut self.systems
    }

    pub fn keys(&self) -> &[SystemKey] {
        &self.keys
    }

    pub fn systems(&self) -> &[SystemObject] {
        &self.systems
    }

    pub fn conflict(&self) -> &ConflictTable {
        &self.conflict
    }

    pub fn incoming(&self) -> &[u32] {
        &self.incoming
    }

    pub fn outgoing(&self) -> &[&[u32]] {
        &self.outgoing
    }

    pub fn condition_incoming(&self) -> &[u32] {
        &self.condition_incoming
    }

    pub fn condition_outgoing(&self) -> &[&[u32]] {
        &self.condition_outgoing
    }
}

// -----------------------------------------------------------------------------
// Allocator Implementation

impl Allocator {
    fn iter(&self) -> impl ExactSizeIterator<Item = (SystemId, SystemKey)> + '_ {
        self.idents.iter().map(|(&x, &y)| (x, y))
    }

    fn len(&self) -> usize {
        self.idents.len()
    }

    fn contains(&self, id: SystemId) -> bool {
        self.idents.contains_key(&id)
    }

    fn get_key(&self, id: SystemId) -> Option<SystemKey> {
        self.idents.get(&id).copied()
    }

    fn get_id(&self, key: SystemKey) -> Option<SystemId> {
        self.slots.get(key).copied()
    }

    fn insert(&mut self, id: SystemId) -> SystemKey {
        self.idents.get(&id).copied().unwrap_or_else(|| {
            let key = self.slots.insert(id);
            self.idents.insert(id, key);
            key
        })
    }

    fn remove(&mut self, id: SystemId) -> Option<SystemKey> {
        let key = self.idents.remove(&id)?;
        let removed = self.slots.remove(key);
        debug_assert_eq!(removed, Some(id));
        Some(key)
    }
}

// -----------------------------------------------------------------------------
// SystemBuffer Implementation

impl SystemBuffer {
    fn insert_action(&mut self, key: SystemKey, system: ActionSystem) {
        let obj = SystemObject::new_action(system);
        self.nodes.insert(key, Some(obj));
        self.uninit.push(key);
    }

    fn insert_condition(&mut self, key: SystemKey, system: ConditionSystem) {
        let obj = SystemObject::new_condition(system);
        self.nodes.insert(key, Some(obj));
        self.uninit.push(key);
    }

    fn remove(&mut self, key: SystemKey) {
        self.nodes.remove(key);

        if let Some(index) = self.uninit.iter().position(|value| *value == key) {
            self.uninit.swap_remove(index);
        }
    }

    fn get_system(&self, key: SystemKey) -> Option<&SystemObject> {
        self.nodes.get(key).and_then(Option::as_ref)
    }

    fn get_system_mut(&mut self, key: SystemKey) -> Option<&mut SystemObject> {
        self.nodes.get_mut(key).and_then(Option::as_mut)
    }

    fn take_system(&mut self, key: SystemKey) -> SystemObject {
        self.nodes.get_mut(key).unwrap().take().unwrap()
    }
}

// -----------------------------------------------------------------------------
// OrderingGraph Implementation

impl OrderingGraph {
    fn insert_order(&mut self, before: SystemKey, after: SystemKey) {
        self.dependency.insert_edge(before, after);
    }

    fn remove_order(&mut self, before: SystemKey, after: SystemKey) -> bool {
        self.dependency.remove_edge(before, after)
    }

    fn insert_condition(&mut self, runnable: SystemKey, condition: SystemKey) {
        self.condition.insert_edge(condition, runnable);
    }

    fn remove_condition(&mut self, runnable: SystemKey, condition: SystemKey) -> bool {
        self.condition.remove_edge(condition, runnable)
    }

    fn insert_node(&mut self, key: SystemKey) {
        self.dependency.insert_node(key);
        self.condition.insert_node(key); // optional
    }

    fn remove_node(&mut self, key: SystemKey) {
        self.dependency.remove_node(key);
        self.condition.remove_node(key);
    }
}

// -----------------------------------------------------------------------------
// ConflictTable Implementation

impl ConflictGraph {
    fn set_exclusive(&mut self, key: SystemKey) {
        self.exclusive.insert(key);
    }

    fn set_conflict(&mut self, a: SystemKey, b: SystemKey) {
        self.conflicts.entry(a).or_default().insert(b);
        self.conflicts.entry(b).or_default().insert(a);
    }

    fn is_exclusive(&self, key: SystemKey) -> bool {
        self.exclusive.contains(&key)
    }

    fn is_conflict(&self, a: SystemKey, b: SystemKey) -> bool {
        self.conflicts.get(&a).is_some_and(|set| set.contains(&b))
    }

    fn remove(&mut self, key: SystemKey) {
        self.exclusive.remove(&key);
        if let Some(a_set) = self.conflicts.remove(&key) {
            a_set.iter().for_each(|b| {
                if let Some(b_set) = self.conflicts.get_mut(b) {
                    b_set.remove(&key);
                }
            });
        }
    }
}

impl ConflictTable {
    fn new(lines: usize) -> Self {
        Self {
            lines,
            table: FixedBitSet::with_capacity(lines * lines),
        }
    }

    pub unsafe fn set_conflict(&mut self, a: u32, b: u32) {
        let index = a as usize * self.lines + b as usize;
        debug_assert!(index <= self.lines * self.lines);
        unsafe { self.table.insert_unchecked(index) }
    }

    pub unsafe fn set_exclusive(&mut self, a: u32) {
        for line in 0..self.lines {
            let index = a as usize * self.lines + line;
            unsafe { self.table.insert_unchecked(index) };
        }
        for line in 0..self.lines {
            let index = a as usize + line * self.lines;
            unsafe { self.table.insert_unchecked(index) };
        }
    }

    #[inline(always)]
    pub unsafe fn is_conflict(&self, a: u32, b: u32) -> bool {
        let index = a as usize * self.lines + b as usize;
        debug_assert!(index <= self.lines * self.lines);
        unsafe { self.table.contains_unchecked(index) }
    }
}

// -----------------------------------------------------------------------------
// Schedule Implementation

impl Debug for Schedule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Schedule")
            .field("label", &self.label)
            .field("systems", &self.allocator.idents.keys())
            .finish()
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new(AnonymousSchedule)
    }
}

impl Schedule {
    fn init_systems(&mut self, world: &mut World) {
        use SystemObject::{Action, Condition};

        let buffer = &mut self.buffer;
        let conflict = &mut self.conflict;

        let uninit = core::mem::take(&mut buffer.uninit);

        // ---------------------------------------------------------------
        // Initialize systems
        uninit.iter().for_each(|&key| {
            if let Some(obj) = buffer.get_system_mut(key) {
                match obj {
                    Action { system, access } => {
                        *access = system.initialize(world);
                    }
                    Condition { system, access } => {
                        *access = system.initialize(world);
                    }
                }
            } else {
                voker_utils::cold_path();
                unreachable!(
                    "A non-existent uninitialized system: {:?}.",
                    self.allocator.get_id(key)
                );
            }
        });

        // ---------------------------------------------------------------
        // Update conflict graph
        uninit.iter().for_each(|&a| {
            let obj = unsafe { buffer.get_system(a).debug_checked_unwrap() };

            let a_access = match obj {
                Action { system, access } => {
                    if system.is_exclusive() {
                        conflict.set_exclusive(a);
                        return; // next loop
                    }
                    access
                }
                Condition { system, access } => {
                    if system.is_exclusive() {
                        conflict.set_exclusive(a);
                        return; // next loop
                    }
                    access
                }
            };

            for (b, v) in buffer.nodes.iter() {
                if let Some(v) = v
                    && a != b
                {
                    match v {
                        Action { access, .. } | Condition { access, .. } => {
                            if !a_access.parallelizable(access) {
                                conflict.set_conflict(a, b);
                            }
                        }
                    }
                }
            }
        });
    }

    fn recycle_schedule(&mut self) {
        let schedule = &mut self.schedule;
        let buffer = &mut self.buffer;

        schedule.conflict.lines = 0;
        schedule.conflict.table.clear();
        schedule.incoming.clear();
        schedule.outgoing.clear();
        schedule.condition_incoming.clear();
        schedule.condition_outgoing.clear();
        schedule.pool = PagePool::new();

        schedule
            .keys
            .drain(..)
            .zip(schedule.systems.drain(..))
            .for_each(|(k, v)| {
                *buffer.nodes.get_mut(k).unwrap() = Some(v);
            });
    }

    fn build_schedule(&mut self) {
        let buffer: &mut SystemBuffer = &mut self.buffer;
        let schedule: &mut SystemSchedule = &mut self.schedule;
        let ordering: &mut OrderingGraph = &mut self.ordering;
        let conflict: &mut ConflictGraph = &mut self.conflict;
        assert!(schedule.keys.is_empty() && schedule.systems.is_empty());
        assert!(schedule.outgoing.is_empty() && schedule.incoming.is_empty());
        assert!(schedule.condition_incoming.is_empty() && schedule.condition_outgoing.is_empty());

        // ---------------------------------------------------------------
        // mix dependency and condition order
        let mut dag = ordering.dependency.clone();
        ordering.condition.all_edges().for_each(|(a, b)| {
            dag.insert_edge(a, b);
        });

        // ---------------------------------------------------------------
        // toposort
        match dag.toposort() {
            Ok(keys) => schedule.keys.extend_from_slice(keys),
            Err(err) => self.handle_toposort_error(err),
        }

        let topo: &[SystemKey] = &schedule.keys;
        schedule
            .systems
            .extend(topo.iter().map(|&key| buffer.take_system(key)));
        debug_assert_eq!(schedule.keys.len(), schedule.systems.len());

        // ---------------------------------------------------------------
        // map key to index
        let mut indices: SparseHashMap<SystemKey, usize> = SparseHashMap::new();
        indices.reserve(topo.len() + (topo.len() >> 1));

        topo.iter().enumerate().for_each(|(idx, &key)| {
            indices.insert(key, idx);
        });

        // ---------------------------------------------------------------
        // calculate incoming and outgoing
        schedule.incoming.resize(topo.len(), 0);
        schedule.outgoing.resize(topo.len(), &[]);
        schedule.condition_incoming.resize(topo.len(), 0);
        schedule.condition_outgoing.resize(topo.len(), &[]);

        let mut outgoing: Vec<Vec<u32>> = Vec::with_capacity(topo.len());
        let mut condition_outgoing: Vec<Vec<u32>> = Vec::with_capacity(topo.len());
        outgoing.resize_with(topo.len(), Vec::<u32>::new);
        condition_outgoing.resize_with(topo.len(), Vec::<u32>::new);

        // explicit and implicit dependencies
        topo.iter().enumerate().for_each(|(idx, &key)| {
            dag.neighbors(key).for_each(|to| {
                let neighbor_index = indices[&to];
                schedule.incoming[neighbor_index] += 1;
                outgoing[idx].push(neighbor_index as u32);
            });
        });
        ::core::mem::drop(dag);

        // condition order
        topo.iter().enumerate().for_each(|(idx, &key)| {
            ordering.condition.neighbors(key).for_each(|to| {
                let neighbor_index = indices[&to];
                schedule.condition_incoming[neighbor_index] += 1;
                condition_outgoing[idx].push(neighbor_index as u32);
            });
        });
        ::core::mem::drop(indices);

        // ---------------------------------------------------------------
        // move data to pool, for compact data allocation.
        schedule.pool = PagePool::new();

        outgoing.iter().enumerate().for_each(|(idx, slice)| {
            let item: &[u32] = schedule.pool.alloc_slice(slice.as_slice());
            schedule.outgoing[idx] = unsafe { core::mem::transmute::<&[u32], &[u32]>(item) };
        });
        ::core::mem::drop(outgoing);

        condition_outgoing.iter().enumerate().for_each(|(idx, slice)| {
            let item: &[u32] = schedule.pool.alloc_slice(slice.as_slice());
            schedule.condition_outgoing[idx] =
                unsafe { core::mem::transmute::<&[u32], &[u32]>(item) };
        });
        ::core::mem::drop(condition_outgoing);

        // ---------------------------------------------------------------
        // build fixed conflict table
        let mut conflict_table = ConflictTable::new(topo.len());

        topo.iter().enumerate().for_each(|(idx_a, &key_a)| {
            if conflict.is_exclusive(key_a) {
                unsafe {
                    conflict_table.set_exclusive(idx_a as u32);
                }
                return; // next loop
            }
            topo[(idx_a + 1)..].iter().enumerate().for_each(|(offset, &key_b)| {
                let idx_b = idx_a + offset + 1;
                if conflict.is_conflict(key_a, key_b) {
                    unsafe {
                        conflict_table.set_conflict(idx_a as u32, idx_b as u32);
                        conflict_table.set_conflict(idx_b as u32, idx_a as u32);
                    }
                }
            });
        });

        schedule.conflict = conflict_table;
    }

    #[cold]
    #[inline(never)]
    #[rustfmt::skip] // More compact.
    fn handle_toposort_error(&mut self, err: ToposortError<SystemKey>) -> ! {
        match err {
            ToposortError::Loop(item) => {
                let name = self.get_id(item).unwrap_or_else(|| {
                    panic!("Update schedule `{:?}` faild: {}.", self.label, ToposortError::Loop(item))
                });
                panic!("Update schedule `{:?}` faild, self-loop detected at node `{name:?}`", self.label);
            },
            ToposortError::Cycle(items) => {
                let names: Vec<Vec<SystemId>> = items.iter().map(|item| {
                    item.iter().filter_map(|key| self.get_id(*key)).collect::<Vec<SystemId>>()
                }).collect();
                panic!("Update schedule `{:?}` faild, cycles detected: `{names:?}`", self.label);
            },
        }
    }

    /// Rebuilds the executable schedule if structure or systems changed.
    ///
    /// This step initializes newly inserted systems, recomputes conflicts,
    /// rebuilds the execution DAG, and initializes the executor if needed.
    ///
    /// Calling this repeatedly without any structural changes is cheap after
    /// the first rebuild.
    pub fn update(&mut self, world: &mut World) {
        if self.is_changed {
            voker_utils::cold_path();
            // self.recycle_schedule();
            self.init_systems(world);
            self.build_schedule();
            self.is_changed = false;
        }

        if !self.executor_initialized {
            voker_utils::cold_path();
            self.executor.init(&self.schedule);
            self.executor_initialized = true;
        }
    }

    /// Executes the schedule once.
    ///
    /// This performs [`Schedule::update`] first, runs all systems through the
    /// configured executor, then updates world ticks and applies deferred
    /// commands.
    ///
    /// Deferred commands are applied after system execution, so world
    /// mutations requested through `Commands` become visible in the next run
    /// unless explicitly flushed/applied by custom flow.
    pub fn run(&mut self, world: &mut World) {
        self.update(world);

        let handler = world.fallback_error_handler();
        self.executor.run(&mut self.schedule, world, handler.0);

        world.advance_tick();
        world.flush();
    }

    /// Creates a new schedule with the given label.
    ///
    /// The concrete executor is selected from [`ExecutorKind::default`].
    pub fn new(label: impl ScheduleLabel) -> Self {
        Self {
            label: label.intern(),
            executor: match ExecutorKind::default() {
                ExecutorKind::SingleThreaded => Box::new(SingleThreadedExecutor::new()),
                ExecutorKind::MultiThreaded => Box::new(MultiThreadedExecutor::new()),
            },
            executor_initialized: false,
            is_changed: false,
            allocator: Default::default(),
            buffer: Default::default(),
            ordering: Default::default(),
            conflict: Default::default(),
            schedule: Default::default(),
            set_anchors: Default::default(),
        }
    }

    /// Creates a new schedule with the given label and given executor.
    pub fn with_executor(
        label: impl ScheduleLabel,
        executor: impl SystemExecutor + 'static,
    ) -> Self {
        Self {
            label: label.intern(),
            allocator: Default::default(),
            buffer: Default::default(),
            ordering: Default::default(),
            conflict: Default::default(),
            schedule: Default::default(),
            set_anchors: Default::default(),
            executor: Box::new(executor),
            executor_initialized: false,
            is_changed: Default::default(),
        }
    }

    /// Returns this schedule's interned label.
    pub fn label(&self) -> InternedScheduleLabel {
        self.label
    }

    /// Returns `true` if a system with `name` exists in this schedule.
    pub fn contains(&self, name: SystemId) -> bool {
        self.allocator.contains(name)
    }

    /// Inserts or replaces a system under `SystemId`.
    ///
    /// Returns `true` if this is a new insertion, `false` if an existing system
    /// with the same Id was replaced.
    pub fn insert_action(&mut self, system: ActionSystem) -> bool {
        let id = system.id();

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        if let Some(key) = self.allocator.get_key(id) {
            self.buffer.remove(key);
            self.buffer.insert_action(key, system);
            let len = self.allocator.len();
            assert!(
                len <= u32::MAX as usize,
                "too many systems in schedule {:?}",
                self.label
            );
            false
        } else {
            let key = self.allocator.insert(id);
            self.buffer.insert_action(key, system);
            self.ordering.insert_node(key);
            let len = self.allocator.len();
            assert!(
                len <= u32::MAX as usize,
                "too many systems in schedule {:?}",
                self.label
            );
            true
        }
    }

    /// Inserts or replaces a system under `SystemId`.
    ///
    /// Returns `true` if this is a new insertion, `false` if an existing system
    /// with the same Id was replaced.
    pub fn insert_condition(&mut self, system: ConditionSystem) -> bool {
        let id = system.id();

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        if let Some(key) = self.allocator.get_key(id) {
            self.buffer.remove(key);
            self.buffer.insert_condition(key, system);
            let len = self.allocator.len();
            assert!(
                len <= u32::MAX as usize,
                "too many systems in schedule {:?}",
                self.label
            );
            false
        } else {
            let key = self.allocator.insert(id);
            self.buffer.insert_condition(key, system);
            self.ordering.insert_node(key);
            let len = self.allocator.len();
            assert!(
                len <= u32::MAX as usize,
                "too many systems in schedule {:?}",
                self.label
            );
            true
        }
    }

    /// Removes a system by name.
    ///
    /// Returns `true` if a system was removed.
    pub fn remove(&mut self, name: SystemId) -> bool {
        let Some(key) = self.allocator.remove(name) else {
            return false;
        };

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        self.buffer.remove(key);
        self.ordering.remove_node(key);
        self.conflict.remove(key);
        self.set_anchors.remove(&name);

        true
    }

    pub fn insert_order(&mut self, before: SystemId, after: SystemId) -> bool {
        let Some(b) = self.allocator.get_key(before) else {
            return false;
        };
        let Some(a) = self.allocator.get_key(after) else {
            return false;
        };

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        self.ordering.insert_order(b, a);

        true
    }

    pub fn remove_order(&mut self, before: SystemId, after: SystemId) -> bool {
        let Some(b) = self.allocator.get_key(before) else {
            return false;
        };
        let Some(a) = self.allocator.get_key(after) else {
            return false;
        };

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        self.ordering.remove_order(b, a)
    }

    pub fn insert_run_if(&mut self, runnable: SystemId, condition: SystemId) -> bool {
        let Some(r) = self.allocator.get_key(runnable) else {
            return false;
        };
        let Some(c) = self.allocator.get_key(condition) else {
            return false;
        };

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        self.ordering.insert_condition(r, c);

        true
    }

    pub fn remove_run_if(&mut self, runnable: SystemId, condition: SystemId) -> bool {
        let Some(r) = self.allocator.get_key(runnable) else {
            return false;
        };
        let Some(c) = self.allocator.get_key(condition) else {
            return false;
        };

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        self.ordering.remove_condition(r, c)
    }

    /// Returns the internal key for a system name.
    pub fn get_key(&self, name: SystemId) -> Option<SystemKey> {
        self.allocator.get_key(name)
    }

    /// Returns the system name for an internal key.
    pub fn get_id(&self, key: SystemKey) -> Option<SystemId> {
        self.allocator.get_id(key)
    }

    /// Iterates over all registered systems as `(name, key)` pairs.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (SystemId, SystemKey)> + '_ {
        self.allocator.iter()
    }

    pub fn dependency_graph(&self) -> &Dag<SystemKey> {
        &self.ordering.dependency
    }

    pub fn condition_graph(&self) -> &Dag<SystemKey> {
        &self.ordering.condition
    }
}

impl Schedule {
    /// Adds a single action system.
    pub fn add_system<S, M>(&mut self, system: S) -> &mut Self
    where
        S: IntoSystem<(), (), M>,
    {
        self.insert_action(Box::new(IntoSystem::into_system(system)));
        self
    }

    /// Removes a single action system.
    pub fn del_system<S, M>(&mut self, system: S) -> &mut Self
    where
        S: IntoSystem<(), (), M>,
    {
        self.remove(system.system_id());
        self
    }

    /// Adds a single condition system.
    pub fn add_condition<S, M>(&mut self, system: S) -> &mut Self
    where
        S: IntoSystem<(), bool, M>,
    {
        self.insert_condition(Box::new(IntoSystem::into_system(system)));
        self
    }

    /// Removes a single condition system.
    pub fn del_condition<S, M>(&mut self, system: S) -> &mut Self
    where
        S: IntoSystem<(), bool, M>,
    {
        self.remove(system.system_id());
        self
    }

    /// Adds one or many systems through [`IntoSystemConfig`].
    ///
    /// This is equivalent to calling [`Schedule::config`]. It accepts a
    /// single system, a condition, a tuple of systems/configs, or an already
    /// built [`SystemConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_ecs::schedule::{IntoSystemConfig, Schedule};
    ///
    /// fn startup() {}
    /// fn gameplay() {}
    /// fn can_run() -> bool { true }
    ///
    /// let mut schedule = Schedule::default();
    ///
    /// schedule.add_systems((startup, gameplay).chain().run_if(can_run));
    /// ```
    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn add_systems<M>(&mut self, config: impl IntoSystemConfig<M>) -> &mut Self {
        self.config(config)
    }

    /// Applies a [`SystemConfig`] into this schedule.
    ///
    /// The provided configuration may insert:
    /// - action and condition systems,
    /// - deferred apply helper systems,
    /// - explicit dependency edges,
    /// - run-condition edges.
    ///
    /// Existing systems with the same `SystemId` are kept and not overwritten.
    ///
    /// # Examples
    ///
    /// ```
    /// use voker_ecs::schedule::{IntoSystemConfig, Schedule};
    ///
    /// fn a() {}
    /// fn b() {}
    ///
    /// let config = a.before(b);
    ///
    /// let mut schedule = Schedule::default();
    /// schedule.config(config);
    /// ```
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn config<M>(&mut self, config: impl IntoSystemConfig<M>) -> &mut Self {
        let SystemConfig {
            systems,
            deferred,
            dependency,
            condition,
            ..
        } = config.into_config();

        for (id, node) in systems {
            if !self.contains(id) {
                match node {
                    SystemNode::Action(system) => {
                        self.insert_action(system);
                    }
                    SystemNode::Condition(system) => {
                        self.insert_condition(system);
                    }
                }
            }
        }

        for (id, apply_deferred) in deferred {
            if !self.contains(id) {
                self.insert_action(apply_deferred);
            }
        }

        for (before, after) in dependency {
            self.insert_order(before, after);
        }

        for (condition, runnable) in condition {
            self.insert_run_if(runnable, condition);
        }

        self
    }
}
