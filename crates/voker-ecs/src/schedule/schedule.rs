//! Core schedule type: system graph construction and execution.
//!
//! A [`Schedule`] owns a directed acyclic graph (DAG) of systems connected by
//! ordering, condition, and set-membership edges. Before first run (and after
//! any structural change) the graph is compiled into a [`SystemSchedule`] —
//! a flattened, executor-ready representation.
//!
//! Main entry points:
//! - [`Schedule::add_systems`] — insert one or many systems into a set.
//! - [`Schedule::config_set`] — apply ordering/condition constraints to a set.
//! - [`Schedule::run`] — compile (if needed) and execute one tick.
#![expect(clippy::module_inception, reason = "For better structure.")]

use core::fmt::Debug;

use alloc::boxed::Box;
use alloc::vec::Vec;

use fixedbitset::FixedBitSet;
use slotmap::{SecondaryMap, SlotMap};
use voker_utils::extra::PagePool;
use voker_utils::hash::{HashMap, HashSet, NoopHashMap, NoopHashSet, SparseHashMap};

use super::IntoSystemConfig;
use super::config::SystemEntry;
use super::set_config::{IntoSystemSetConfig, SystemSetConfig};
use super::{ActionSystem, ConditionSystem, SystemKey, SystemObject};
use super::{AnonymousSchedule, SystemConfig, ToposortError};
use super::{Dag, InternedScheduleLabel, ScheduleLabel, SystemExecutor};
use super::{ExecutorKind, MultiThreadedExecutor, SingleThreadedExecutor};
use crate::system::{InternedSystemSet, SystemSet};
use crate::system::{IntoSystem, System, SystemId};
use crate::tick::Tick;
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
    system_sets: SystemSets,
    set_hierarchy: SetHierarchy,
    executor: Box<dyn SystemExecutor>,
    executor_initialized: bool,
    is_changed: bool,
}

// -----------------------------------------------------------------------------
// SystemSets

type SystemSets = HashMap<InternedSystemSet, NoopHashSet<SystemId>>;

// -----------------------------------------------------------------------------
// SetHierarchy

/// Tracks parent–child relationships between system sets for recursive removal.
///
/// The anonymous set `()` is the implicit root. Sets without an explicit parent
/// are direct children of `()`.
#[derive(Default)]
struct SetHierarchy {
    children: HashMap<InternedSystemSet, NoopHashSet<InternedSystemSet>>,
    parents: HashMap<InternedSystemSet, InternedSystemSet>,
}

// -----------------------------------------------------------------------------
// Allocator

#[derive(Default)]
struct Allocator {
    slots: SlotMap<SystemKey, SystemId>,
    idents: NoopHashMap<SystemId, SystemKey>,
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
    /// Stable system keys aligned with all other columns.
    pub keys: &'s [SystemKey],
    /// Mutable access to compiled system objects.
    pub systems: &'s mut [SystemObject],
    /// Conflict lookup table used by the executor.
    pub conflict: &'s ConflictTable,
    /// Number of normal dependency predecessors for each system index.
    pub incoming: &'s [u32],
    /// Normal dependency adjacency by system index.
    pub outgoing: &'s [&'s [u32]],
    /// Number of run-condition predecessors for each system index.
    pub condition_incoming: &'s [u32],
    /// Run-condition adjacency by system index.
    pub condition_outgoing: &'s [&'s [u32]],
}

impl SystemSchedule {
    /// Returns a structured view over compiled schedule data.
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

    /// Returns mutable access to compiled systems.
    pub fn systems_mut(&mut self) -> &mut [SystemObject] {
        &mut self.systems
    }

    /// Returns compiled system keys.
    pub fn keys(&self) -> &[SystemKey] {
        &self.keys
    }

    /// Returns compiled systems in execution-index order.
    pub fn systems(&self) -> &[SystemObject] {
        &self.systems
    }

    /// Returns the conflict lookup table.
    pub fn conflict(&self) -> &ConflictTable {
        &self.conflict
    }

    /// Returns normal dependency incoming counts.
    pub fn incoming(&self) -> &[u32] {
        &self.incoming
    }

    /// Returns normal dependency adjacency lists.
    pub fn outgoing(&self) -> &[&[u32]] {
        &self.outgoing
    }

    /// Returns run-condition dependency incoming counts.
    pub fn condition_incoming(&self) -> &[u32] {
        &self.condition_incoming
    }

    /// Returns run-condition dependency adjacency lists.
    pub fn condition_outgoing(&self) -> &[&[u32]] {
        &self.condition_outgoing
    }
}

// -----------------------------------------------------------------------------
// Allocator Implementation

impl Allocator {
    fn len(&self) -> usize {
        self.idents.len()
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

    fn insert_condition_order(&mut self, condition: SystemKey, runnable: SystemKey) {
        self.condition.insert_edge(condition, runnable);
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

    /// Marks `(a, b)` as conflicting in the table.
    ///
    /// # Safety
    /// `a` and `b` must be valid matrix indices in `[0, self.lines)`.
    pub unsafe fn set_conflict(&mut self, a: u32, b: u32) {
        let index = a as usize * self.lines + b as usize;
        debug_assert!(index <= self.lines * self.lines);
        unsafe { self.table.insert_unchecked(index) }
    }

    /// Marks every pair involving `a` as conflicting.
    ///
    /// This is used for exclusive systems.
    ///
    /// # Safety
    /// `a` must be a valid matrix index in `[0, self.lines)`.
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

    /// Returns whether `(a, b)` conflicts.
    ///
    /// # Safety
    /// `a` and `b` must be valid matrix indices in `[0, self.lines)`.
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

// -----------------------------------------------------------------------------
// Internal build pipeline

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
                core::hint::cold_path();
                unreachable!(
                    "A non-existent uninitialized system: {:?}.",
                    self.allocator.get_id(key)
                );
            }
        });

        // ---------------------------------------------------------------
        // Update conflict graph
        uninit.iter().for_each(|&a| {
            let obj = buffer.get_system(a).unwrap();

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
            core::hint::cold_path();
            // self.recycle_schedule();
            self.init_systems(world);
            self.build_schedule();
            self.is_changed = false;
        }

        if !self.executor_initialized {
            core::hint::cold_path();
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
            system_sets: Default::default(),
            set_hierarchy: Default::default(),
            buffer: Default::default(),
            ordering: Default::default(),
            conflict: Default::default(),
            schedule: Default::default(),
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
            system_sets: Default::default(),
            set_hierarchy: Default::default(),
            buffer: Default::default(),
            ordering: Default::default(),
            conflict: Default::default(),
            schedule: Default::default(),
            executor: Box::new(executor),
            executor_initialized: false,
            is_changed: Default::default(),
        }
    }

    /// Returns this schedule's interned label.
    pub fn label(&self) -> InternedScheduleLabel {
        self.label
    }

    /// Iterates all registered system ids in this schedule.
    ///
    /// This includes set boundary marker systems (`begin`/`end`) created by
    /// [`SystemSet`] integration.
    pub fn systems(&self) -> impl ExactSizeIterator<Item = SystemId> + '_ {
        self.allocator.idents.keys().copied()
    }

    /// Iterates all currently known system sets.
    ///
    /// A set exists once it is explicitly initialized or when at least one
    /// system is inserted with that set.
    pub fn system_sets(&self) -> impl ExactSizeIterator<Item = InternedSystemSet> + '_ {
        self.system_sets.keys().copied()
    }

    /// Returns `true` if a system id is registered.
    pub fn contains_system(&self, name: SystemId) -> bool {
        self.allocator.idents.contains_key(&name)
    }

    /// Returns `true` if a system set has internal state in this schedule.
    pub fn contains_system_set(&self, name: InternedSystemSet) -> bool {
        self.system_sets.contains_key(&name)
    }

    /// Wires set-boundary condition edges for a newly inserted system.
    ///
    /// Ensures begin/end markers exist, then inserts:
    /// - `begin → system_key` (begin is a condition for the system)
    /// - `end → system_key`   (end depends on the system)
    /// - `end → begin`        (keeps an empty-set boundary)
    fn wire_set_boundaries(&mut self, system_key: SystemKey, id: SystemId, set: InternedSystemSet) {
        let set_members = self.system_sets.entry(set).or_default();
        set_members.insert(id);

        let begin = {
            let marker = set.begin();
            let id = marker.id();
            self.allocator.get_key(id).unwrap_or_else(|| {
                let key = self.allocator.insert(id);
                self.buffer.insert_action(key, marker);
                self.ordering.insert_node(key);
                set_members.insert(id);
                key
            })
        };

        let end = {
            let marker = set.end();
            let id = marker.id();
            self.allocator.get_key(id).unwrap_or_else(|| {
                let key = self.allocator.insert(id);
                self.buffer.insert_action(key, marker);
                self.ordering.insert_node(key);
                set_members.insert(id);
                key
            })
        };

        self.ordering.insert_order(system_key, end);
        self.ordering.insert_condition_order(begin, system_key);
        self.ordering.insert_condition_order(begin, end);
    }

    /// Inserts (or replaces) an action system and associates it with `set`.
    ///
    /// This also ensures the set boundary marker systems exist and wires
    /// run-condition edges so the system executes within `set` boundaries.
    fn insert_action(&mut self, system: ActionSystem, set: InternedSystemSet) {
        let id = system.id();

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        let system_key = self.allocator.get_key(id).unwrap_or_else(|| {
            let key = self.allocator.insert(id);
            self.buffer.insert_action(key, system);
            self.ordering.insert_node(key);
            key
        });

        self.wire_set_boundaries(system_key, id, set);

        assert!(
            self.allocator.len() <= u32::MAX as usize,
            "too many systems in schedule {:?}",
            self.label
        );
    }

    /// Inserts (or replaces) a condition system and associates it with `set`.
    ///
    /// This also ensures the set boundary marker systems exist and wires
    /// run-condition edges so the condition is evaluated within `set`
    /// boundaries.
    fn insert_condition(&mut self, system: ConditionSystem, set: InternedSystemSet) {
        let id = system.id();

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        let system_key = self.allocator.get_key(id).unwrap_or_else(|| {
            let key = self.allocator.insert(id);
            self.buffer.insert_condition(key, system);
            self.ordering.insert_node(key);
            key
        });

        self.wire_set_boundaries(system_key, id, set);

        assert!(
            self.allocator.len() <= u32::MAX as usize,
            "too many systems in schedule {:?}",
            self.label
        );
    }

    /// Ensures `set` exists in this schedule.
    ///
    /// If absent, this inserts the set's begin/end markers and a direct
    /// `begin -> end` condition edge.
    fn init_system_set(&mut self, set: InternedSystemSet) {
        if !self.system_sets.contains_key(&set) {
            if !self.is_changed {
                self.recycle_schedule();
                self.is_changed = true;
            }

            let sets = self.system_sets.entry(set).or_default();

            let begin_system = set.begin();
            let begin_id = begin_system.id();
            let begin = self.allocator.get_key(begin_id).unwrap_or_else(|| {
                let key = self.allocator.insert(begin_id);
                self.buffer.insert_action(key, begin_system);
                self.ordering.insert_node(key);
                sets.insert(begin_id);
                key
            });

            let end_system = set.end();
            let end_id = end_system.id();
            let end = self.allocator.get_key(end_id).unwrap_or_else(|| {
                let key = self.allocator.insert(end_id);
                self.buffer.insert_action(key, end_system);
                self.ordering.insert_node(key);
                sets.insert(end_id);
                key
            });
            self.ordering.insert_condition_order(begin, end);
        }
    }

    /// Removes `set` and all systems currently tracked inside it.
    ///
    /// This includes set boundary marker systems.
    fn remove_system_set_inner(&mut self, set: InternedSystemSet) {
        if let Some(sets) = self.system_sets.remove(&set) {
            for id in sets {
                self.remove(id);
            }
        }
    }

    /// Removes a system by name.
    ///
    /// Returns `true` if a system was removed.
    fn remove(&mut self, name: SystemId) -> bool {
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
        for set in self.system_sets.values_mut() {
            set.remove(&name);
        }

        true
    }

    /// Inserts an explicit dependency edge `before -> after`.
    ///
    /// Returns `false` if either system is not registered.
    fn insert_order(&mut self, before: SystemId, after: SystemId) -> bool {
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

    /// Adds a run-condition edge `condition -> runnable`.
    ///
    /// Returns `false` if either system is not registered.
    fn insert_run_if(&mut self, runnable: SystemId, condition: SystemId) -> bool {
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

        self.ordering.insert_condition_order(c, r);

        true
    }

    /// Returns the internal key for a system name.
    pub fn get_key(&self, name: SystemId) -> Option<SystemKey> {
        self.allocator.get_key(name)
    }

    /// Returns the system name for an internal key.
    pub fn get_id(&self, key: SystemKey) -> Option<SystemId> {
        self.allocator.get_id(key)
    }

    /// Returns the explicit dependency graph.
    pub fn dependency_graph(&self) -> &Dag<SystemKey> {
        &self.ordering.dependency
    }

    /// Returns the run-condition graph.
    pub fn condition_graph(&self) -> &Dag<SystemKey> {
        &self.ordering.condition
    }

    pub(crate) fn check_ticks(&mut self, now: Tick) {
        for system in self.schedule.systems.iter_mut() {
            system.check_ticks(now);
        }
    }
}

// -----------------------------------------------------------------------------
// Public API

impl Schedule {
    /// Adds one or many systems into `set`.
    ///
    /// All systems in `config` have their [`SystemId`] updated to include `set`
    /// membership via [`System::set_system_set`], ensuring identity uniqueness
    /// across sets. The set's begin/end boundary markers are created if needed.
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
    /// schedule.add_systems((), (startup, gameplay).chain().run_if(can_run));
    /// ```
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    pub fn add_systems<M>(
        &mut self,
        set: impl SystemSet,
        config: impl IntoSystemConfig<M>,
    ) -> &mut Self {
        let set = set.intern();
        self.init_system_set(set);
        let mut cfg = config.into_config();
        cfg.apply_to_set(set);

        let SystemConfig {
            systems,
            deferred,
            dependencies,
            conditions,
            ..
        } = cfg;

        for (_, entry) in systems {
            match entry {
                SystemEntry::Action(system) => self.insert_action(system, set),
                SystemEntry::Condition(system) => self.insert_condition(system, set),
            }
        }

        for (id, apply) in deferred {
            let def_id = apply.id();
            self.insert_action(apply, set);
            self.insert_order(id, def_id);
        }

        for (before, after) in dependencies {
            self.insert_order(before, after);
        }

        for (condition, runnable) in conditions {
            self.insert_run_if(runnable, condition);
        }

        self
    }

    pub fn add_system<M>(
        &mut self,
        set: impl SystemSet,
        system: impl IntoSystem<(), (), M>,
    ) -> &mut Self {
        let mut system = IntoSystem::into_system(system);
        let set = set.intern();
        self.init_system_set(set);
        system.set_system_set(set);
        self.insert_action(Box::new(system), set);
        self
    }

    /// Removes `set` and all its children recursively.
    ///
    /// Children are removed depth-first before the parent, so all membership
    /// and boundary edges are cleaned up in order.
    pub fn remove_system_set(&mut self, set: impl SystemSet) -> &mut Self {
        let set = set.intern();
        self.remove_set_recursive(set);
        self
    }

    fn remove_set_recursive(&mut self, set: InternedSystemSet) {
        if let Some(children) = self.set_hierarchy.children.remove(&set) {
            for child in children {
                self.set_hierarchy.parents.remove(&child);
                self.remove_set_recursive(child);
            }
        }
        self.set_hierarchy.parents.remove(&set);
        self.remove_system_set_inner(set);
    }

    /// Applies a [`SystemSetConfig`] to this schedule.
    ///
    /// This can initialize a set, establish parent–child nesting, add ordering
    /// relative to other sets, and attach run conditions to an entire set.
    pub fn config_set(&mut self, config: impl IntoSystemSetConfig) -> &mut Self {
        let SystemSetConfig {
            set,
            parent,
            run_after,
            run_before,
            conditions,
        } = config.into_set_config();

        self.init_system_set(set);

        if let Some(parent) = parent {
            self.init_system_set(parent);
            // Wire child's begin and end markers into the parent set so they
            // run within the parent's execution window.
            let begin: ActionSystem = set.begin();
            let end: ActionSystem = set.end();
            self.insert_action(begin, parent);
            self.insert_action(end, parent);
            self.set_hierarchy.children.entry(parent).or_default().insert(set);
            self.set_hierarchy.parents.insert(set, parent);
        }

        for other in run_after {
            self.init_system_set(other);
            let other_end_id = other.end().id();
            let self_begin_id = set.begin().id();
            self.insert_order(other_end_id, self_begin_id);
        }

        for other in run_before {
            self.init_system_set(other);
            let self_end_id = set.end().id();
            let other_begin_id = other.begin().id();
            self.insert_order(self_end_id, other_begin_id);
        }

        for cond in conditions {
            let cond_id = cond.id();
            self.insert_condition(cond, ().intern());
            let begin_id = set.begin().id();
            self.insert_run_if(begin_id, cond_id);
        }

        self
    }
}
