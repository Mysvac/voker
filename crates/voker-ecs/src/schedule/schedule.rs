#![expect(clippy::module_inception, reason = "For better structure.")]

use core::fmt::Debug;

use alloc::boxed::Box;
use alloc::vec::Vec;

use fixedbitset::FixedBitSet;
use slotmap::{SecondaryMap, SlotMap};
use voker_utils::extra::PagePool;
use voker_utils::hash::{HashMap, HashSet, NoOpHashMap};

use super::{Dag, SystemKey, SystemObject, UnitSystem};
use super::{ExecutorKind, MultiThreadedExecutor, SingleThreadedExecutor};
use super::{InternedScheduleLabel, ScheduleLabel, SystemExecutor};
use crate::schedule::AnonymousSchedule;
use crate::system::{IntoSystem, SystemId};
use crate::world::World;

// -----------------------------------------------------------------------------
// Schedule

/// A schedulable collection of systems with ordering and conflict constraints.
///
/// `Schedule` stores systems, explicit ordering edges, and access metadata, then
/// compiles them into an executable graph.
///
/// # Execution Graph and Parallelism
///
/// On [`Schedule::update`], when the schedule is marked as changed:
/// - New/updated systems are initialized to collect their [`AccessTable`].
/// - Pairwise access conflicts are recorded.
/// - User-provided ordering edges are merged with conflict/exclusive constraints.
/// - A reduced DAG is built and converted into compact runtime arrays
///   (`incoming` / `outgoing`) for the executor.
///
/// During [`Schedule::run`], the selected executor uses that DAG to run
/// independent systems in parallel while respecting:
/// - explicit ordering constraints,
/// - access conflicts,
/// - and exclusive systems.
///
/// # Change Tracking
///
/// Structural edits such as system insertion/removal and explicit order edits
/// mark the schedule as changed. The next [`Schedule::update`] (or [`Schedule::run`])
/// will rebuild the compiled graph.
///
/// [`AccessTable`]: crate::system::AccessTable
pub struct Schedule {
    label: InternedScheduleLabel,
    allocator: Allocator,
    buffer: SystemBuffer,
    ordering: OrderingGraph,
    conflict: ConflictTable,
    schedule: SystemSchedule,
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
    ordering: Dag<SystemKey>,
}

// -----------------------------------------------------------------------------
// ConflictTable

#[derive(Default)]
struct ConflictTable {
    exclusive: HashSet<SystemKey>,
    conflicts: HashMap<SystemKey, HashSet<SystemKey>>,
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
    /// Collection of system keys
    keys: Vec<SystemKey>,
    /// Collection of system objects
    systems: Vec<SystemObject>,
    /// In-degree of each system in the execution graph
    incoming: Vec<u16>,
    /// Successor nodes of each system in the execution graph
    ///
    /// When a system completes, we iterate through its successors and decrement
    /// their in-degree. Systems with an in-degree of zero are ready to run.
    ///
    /// A local memory pool is used here to manage data and avoid excessive
    /// memory fragmentation.
    outgoing: Vec<&'static [u16]>,
    pool: PagePool,
}

unsafe impl Sync for SystemSchedule {}
unsafe impl Send for SystemSchedule {}

/// View of `SystemSchedule` for encapsulating internal implementation
pub struct SystemScheduleView<'s> {
    pub keys: &'s [SystemKey],
    pub systems: &'s mut [SystemObject],
    pub incoming: &'s [u16],
    pub outgoing: &'s [&'s [u16]],
}

impl SystemSchedule {
    pub fn view(&mut self) -> SystemScheduleView<'_> {
        let SystemSchedule {
            keys,
            systems,
            incoming,
            outgoing,
            ..
        } = self;

        SystemScheduleView {
            keys,
            systems,
            incoming,
            outgoing,
        }
    }

    pub fn keys(&self) -> &[SystemKey] {
        &self.keys
    }

    pub fn systems(&self) -> &[SystemObject] {
        &self.systems
    }

    pub fn incoming(&self) -> &[u16] {
        &self.incoming
    }

    pub fn outgoing(&self) -> &[&[u16]] {
        &self.outgoing
    }
}

// -----------------------------------------------------------------------------
// Allocator Implementation

impl Allocator {
    fn iter(&self) -> impl ExactSizeIterator<Item = (&SystemId, &SystemKey)> + '_ {
        self.idents.iter()
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

    fn get_name(&self, key: SystemKey) -> Option<SystemId> {
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
    fn insert(&mut self, key: SystemKey, system: UnitSystem) {
        let obj = SystemObject::new_uninit(system);
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
    fn insert(&mut self, a: SystemKey, b: SystemKey) {
        self.ordering.insert_edge(a, b);
    }

    fn remove(&mut self, a: SystemKey, b: SystemKey) -> bool {
        self.ordering.remove_edge(a, b)
    }

    fn insert_node(&mut self, key: SystemKey) {
        self.ordering.insert_node(key)
    }

    fn remove_node(&mut self, key: SystemKey) {
        self.ordering.remove_node(key)
    }
}

// -----------------------------------------------------------------------------
// ConflictTable Implementation

impl ConflictTable {
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
        let buffer = &mut self.buffer;
        let conflict = &mut self.conflict;

        let uninit = core::mem::take(&mut buffer.uninit);

        uninit.iter().for_each(|&key| {
            if let Some(obj) = buffer.get_system_mut(key) {
                obj.access = obj.system.initialize(world);
            } else {
                voker_utils::cold_path();
                unreachable!();
            }
        });

        uninit.iter().for_each(|&a| {
            if let Some(obj) = buffer.get_system(a) {
                if obj.system.is_exclusive() {
                    conflict.set_exclusive(a);
                } else {
                    for (b, v) in buffer.nodes.iter() {
                        if let Some(v) = v
                            && a != b
                            && !obj.access.parallelizable(&v.access)
                        {
                            conflict.set_conflict(a, b);
                        }
                    }
                }
            } else {
                voker_utils::cold_path();
                unreachable!();
            }
        });
    }

    fn recycle_schedule(&mut self) {
        let schedule = &mut self.schedule;
        let buffer = &mut self.buffer;
        schedule.incoming.clear();
        schedule.outgoing.clear();
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
        let buffer = &mut self.buffer;
        let schedule = &mut self.schedule;
        let conflict = &mut self.conflict;
        let ordering = &mut self.ordering;
        assert!(schedule.keys.is_empty() && schedule.systems.is_empty());
        assert!(schedule.outgoing.is_empty() && schedule.incoming.is_empty());

        let mut dag = transitive_reduction(conflict, ordering);

        schedule.keys.extend(dag.toposort().unwrap());
        let topo: &[SystemKey] = &schedule.keys;

        schedule.systems.extend(topo.iter().map(|&key| buffer.take_system(key)));
        debug_assert_eq!(schedule.keys.len(), schedule.systems.len());

        schedule.incoming.resize(topo.len(), 0);
        schedule.outgoing.resize(topo.len(), &[]);
        let mut outgoing: Vec<Vec<u16>> = Vec::with_capacity(topo.len());
        outgoing.resize_with(topo.len(), Vec::<u16>::new);

        let mut indices: HashMap<SystemKey, usize> = HashMap::with_capacity(topo.len());
        topo.iter().enumerate().for_each(|(idx, &key)| {
            indices.insert(key, idx);
        });

        topo.iter().enumerate().for_each(|(idx, &key)| {
            dag.neighbors(key).for_each(|to| {
                let neighbor_index = indices[&to];
                schedule.incoming[neighbor_index] += 1;
                outgoing[idx].push(neighbor_index as u16);
            });
        });

        schedule.pool = PagePool::new();
        outgoing.iter().enumerate().for_each(|(idx, slice)| {
            let item: &[u16] = schedule.pool.alloc_slice(slice.as_slice());

            schedule.outgoing[idx] =
                unsafe { core::mem::transmute::<&[u16], &'static [u16]>(item) };
        });
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

        let handler = world.default_error_handler();
        self.executor.run(&mut self.schedule, world, handler.0);

        world.update_tick();
        world.apply_commands();
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

        true
    }

    /// Inserts or replaces a system under `name`.
    ///
    /// Returns `true` if this is a new insertion, `false` if an existing system
    /// with the same name was replaced.
    pub fn insert(&mut self, system: UnitSystem) -> bool {
        let id = system.id();

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        if let Some(key) = self.allocator.get_key(id) {
            self.buffer.remove(key);
            self.buffer.insert(key, system);
            let len = self.allocator.len();
            assert!(
                len <= u16::MAX as usize,
                "too many systems in schedule {:?}",
                self.label
            );
            false
        } else {
            let key = self.allocator.insert(id);
            self.buffer.insert(key, system);
            self.ordering.insert_node(key);
            let len = self.allocator.len();
            assert!(
                len <= u16::MAX as usize,
                "too many systems in schedule {:?}",
                self.label
            );
            true
        }
    }

    /// Adds an explicit ordering edge: `before -> after`.
    ///
    /// Returns `false` if either system name is not present.
    ///
    /// If the edge already exists, this is idempotent.
    pub fn insert_order(&mut self, before: SystemId, after: SystemId) -> bool {
        let Some(a) = self.allocator.get_key(before) else {
            return false;
        };
        let Some(b) = self.allocator.get_key(after) else {
            return false;
        };

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        self.ordering.insert(a, b);

        true
    }

    /// Removes an explicit ordering edge: `before -> after`.
    ///
    /// Returns `false` if either system name is not present or the order is not present.
    pub fn remove_order(&mut self, before: SystemId, after: SystemId) -> bool {
        let Some(a) = self.allocator.get_key(before) else {
            return false;
        };
        let Some(b) = self.allocator.get_key(after) else {
            return false;
        };

        if !self.is_changed {
            self.recycle_schedule();
            self.is_changed = true;
        }

        self.ordering.remove(a, b)
    }

    /// Returns the internal key for a system name.
    pub fn get_key(&self, name: SystemId) -> Option<SystemKey> {
        self.allocator.get_key(name)
    }

    /// Returns the system name for an internal key.
    pub fn get_name(&self, key: SystemKey) -> Option<SystemId> {
        self.allocator.get_name(key)
    }

    /// Iterates over all registered systems as `(name, key)` pairs.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&SystemId, &SystemKey)> + '_ {
        self.allocator.iter()
    }

    /// Returns the explicit ordering graph (without conflict-derived edges).
    pub fn order_graph(&self) -> &Dag<SystemKey> {
        &self.ordering.ordering
    }
}

impl Schedule {
    /// Adds a system using its Rust type name as [`SystemId`].
    pub fn add_system<S, M>(&mut self, system: S) -> &mut Self
    where
        S: IntoSystem<(), (), M>,
    {
        self.insert(Box::new(IntoSystem::into_system(system)));
        self
    }

    pub fn del_system<S, M>(&mut self, system: S) -> &mut Self
    where
        S: IntoSystem<(), (), M>,
    {
        self.remove(system.system_id());
        self
    }

    pub fn add_order<X, Y, M1, M2>(&mut self, before: X, after: Y) -> &mut Self
    where
        X: IntoSystem<(), (), M1>,
        Y: IntoSystem<(), (), M2>,
    {
        self.insert_order(before.system_id(), after.system_id());
        self
    }

    pub fn del_order<X, Y, M1, M2>(&mut self, before: X, after: Y) -> &mut Self
    where
        X: IntoSystem<(), (), M1>,
        Y: IntoSystem<(), (), M2>,
    {
        self.remove_order(before.system_id(), after.system_id());
        self
    }
}

fn transitive_reduction(conflict: &ConflictTable, ordering: &mut OrderingGraph) -> Dag<SystemKey> {
    const fn bind_index(row: usize, col: usize) -> usize {
        // 0
        // 1 2
        // 3 4 5
        ((row * (row + 1)) >> 1) + col
    }

    let (topo, graph) = ordering.ordering.toposort_and_graph().unwrap();
    debug_assert!(topo.len() <= u16::MAX as usize);
    if topo.is_empty() {
        return Dag::new();
    }

    let mut exec_dag = graph.clone();
    let mut index_map = HashMap::<SystemKey, usize>::with_capacity(topo.len());
    index_map.extend(topo.iter().enumerate().map(|(idx, &key)| (key, idx)));

    let system_count = topo.len();
    let mut exclusive_systems = FixedBitSet::with_capacity(system_count);
    let matrix_size = system_count * (system_count + 1) / 2;
    let mut transitive_closure = FixedBitSet::with_capacity(matrix_size);
    topo.iter().enumerate().for_each(|(ib, &kb)| {
        let b_is_exclusive = conflict.is_exclusive(kb);
        if b_is_exclusive {
            unsafe {
                exclusive_systems.insert_unchecked(ib);
            }
        }

        exec_dag.neighbors(kb).for_each(|km| {
            let im = *index_map.get(&km).unwrap();
            unsafe {
                transitive_closure.insert_unchecked(bind_index(im, ib));
            }
        });

        topo[0..ib].iter().enumerate().rev().for_each(|(ia, &ka)| {
            let matrix_index = bind_index(ia, ib);
            if unsafe { transitive_closure.contains_unchecked(matrix_index) } {
                return;
            }

            if b_is_exclusive
                || unsafe { exclusive_systems.contains_unchecked(ia) }
                || conflict.is_conflict(ka, kb)
            {
                unsafe {
                    transitive_closure.insert_unchecked(matrix_index);
                }
                let is_unreachable = exec_dag.neighbors(kb).all(|km| {
                    let im = *index_map.get(&km).unwrap();
                    unsafe { !transitive_closure.contains_unchecked(bind_index(im, ib)) }
                });

                if is_unreachable {
                    exec_dag.insert_edge(ka, kb);
                }
            }
        });
    });

    exec_dag.into()
}
