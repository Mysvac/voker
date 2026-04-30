#![expect(clippy::module_inception, reason = "For better structure.")]

use alloc::boxed::Box;
use core::fmt::Debug;
use core::sync::atomic::Ordering;
use voker_task::ComputeTaskPool;

use voker_os::atomic::AtomicU32;
use voker_os::utils::CachePadded;

use super::WorldId;
use crate::archetype::Archetypes;
use crate::bundle::Bundles;
use crate::command::CommandQueue;
use crate::component::Components;
use crate::entity::{Entities, EntityAllocator};
use crate::error::FallbackErrorHandler;
use crate::event::Events;
use crate::message::Messages;
use crate::observer::Observers;
use crate::resource::Resources;
use crate::schedule::Schedules;
use crate::storage::Storages;
use crate::tick::{CHECK_CYCLE, CheckTicks, Tick};

// -----------------------------------------------------------------------------
// World

/// Central container for ECS runtime state.
///
/// A world owns entity metadata, component/resource storages, scheduling state,
/// deferred commands, and message lifecycle infrastructure.
pub struct World {
    id: WorldId,
    this_run: CachePadded<AtomicU32>,
    last_run: Tick,
    last_check: Tick,
    thread_hash: u64,
    pub(crate) last_trigger: u32,
    pub entities: Entities,
    pub allocator: EntityAllocator,
    pub bundles: Bundles,
    pub archetypes: Archetypes,
    pub components: Components,
    pub resources: Resources,
    pub schedules: Schedules,
    pub storages: Storages,
    pub messages: Messages,
    pub events: Events,
    pub observers: Observers,
    pub command_queue: CommandQueue,
}

unsafe impl Send for World {}
unsafe impl Sync for World {}

impl Debug for World {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("World")
            .field("id", &self.id())
            .field("this_run", &self.this_run())
            .field("last_run", &self.last_run())
            .field("thread_hash", &self.thread_hash())
            .field("entity_count", &self.entity_count())
            .field("resources", &self.resources)
            .field("components", &self.components)
            .field("archetypes", &self.archetypes)
            .field("storages", &self.storages)
            .field("schedules", &self.schedules)
            .finish()
    }
}

impl World {
    /// Creates a new world with the given unique id.
    pub fn alloc() -> Box<World> {
        Box::new(Self {
            id: WorldId::alloc(),
            this_run: CachePadded::new(AtomicU32::new(1)),
            last_run: Tick::new(0),
            last_check: Tick::new(0),
            last_trigger: 0,
            thread_hash: voker_os::thread::thread_hash(),
            entities: Entities::new(),
            allocator: EntityAllocator::new(),
            bundles: Bundles::new(),
            archetypes: Archetypes::new(),
            components: Components::new(),
            resources: Resources::new(),
            schedules: Schedules::new(),
            storages: Storages::new(),
            messages: Messages::new(),
            events: Events::new(),
            observers: Observers::new(),
            command_queue: CommandQueue::new(),
        })
    }

    /// Returns this world's unique id.
    pub fn id(&self) -> WorldId {
        self.id
    }

    /// Returns the thread hash captured when the world was created.
    pub fn thread_hash(&self) -> u64 {
        self.thread_hash
    }

    /// Returns the tick used as `last_run` for change detection.
    pub fn last_run(&self) -> Tick {
        self.last_run
    }

    /// Returns the current world tick (`this_run`).
    pub fn this_run(&self) -> Tick {
        Tick::new(self.this_run.load(Ordering::Relaxed))
    }

    /// Returns the current world tick (`this_run`).
    ///
    /// Requires a mutable borrow of World in `full_mut` state (not `data_mut` state).
    pub fn this_run_fast(&mut self) -> Tick {
        Tick::new(*self.this_run.get_mut())
    }
}

// -----------------------------------------------------------------------------
// Tick

impl World {
    /// An incremental trigger counter used to prevent observers
    /// from triggering repeatedly at the same time.
    #[inline]
    pub fn last_trigger(&self) -> u32 {
        self.last_trigger
    }

    /// Advances `this_run` atomically and returns the previous tick value.
    ///
    /// This is primarily used by concurrent execution paths.
    pub fn advance_tick(&self) -> Tick {
        Tick::new(self.this_run.fetch_add(1, Ordering::Relaxed))
    }

    /// Resets the world's own change-detection baseline.
    ///
    /// After calling this, changes that happened before the current moment are
    /// no longer considered "new" from the world's perspective.
    ///
    /// This only affects the world's internal change tracking. It does not
    /// modify `last_run` values stored inside systems.
    ///
    /// Both systems and the world track changes using a `last_run` marker:
    /// a change is considered visible when it falls within `last_run..this_run`.
    ///
    /// Systems update their own `last_run` automatically after each run, while
    /// the world baseline must be reset manually. This function synchronizes the
    /// world baseline to the current tick.
    pub fn reset_last_run(&mut self) -> Tick {
        self.check_ticks();

        let last_run = *self.this_run.get_mut();
        let this_run = last_run.wrapping_add(1);

        self.last_run = Tick::new(last_run);
        *self.this_run.get_mut() = this_run;

        self.check_ticks();

        Tick::new(this_run)
    }

    /// Runs periodic tick-age validation across component/resource storages.
    ///
    /// Validation runs at most once per [`CHECK_CYCLE`] ticks, measured from
    /// the previous validation point (`last_check`).
    ///
    /// Returns the [`CheckTicks`] event payload used for this pass.
    #[inline]
    pub fn check_ticks(&mut self) -> Option<CheckTicks> {
        #[cold]
        #[inline(never)]
        fn check_ticks_cold(world: &mut World) -> CheckTicks {
            let now = world.this_run_fast();
            let event = CheckTicks::new(now);

            const TASK_POOL: bool = voker_task::cfg::multi_threaded!();

            let schedules = &mut world.schedules;
            let resources = &mut world.storages.resources;
            let tables = &mut world.storages.tables;
            let maps = &mut world.storages.maps;

            if TASK_POOL && let Some(pool) = ComputeTaskPool::try_get() {
                pool.scope(|s| {
                    for (_, sche) in schedules.iter_mut() {
                        s.spawn(async move {
                            sche.check_ticks(now);
                        });
                    }
                    s.spawn(async move {
                        resources.check_ticks(now);
                    });
                    for table in tables.iter_mut() {
                        s.spawn(async move {
                            table.check_ticks(now);
                        });
                    }
                    for map in maps.iter_mut() {
                        s.spawn(async move {
                            map.check_ticks(now);
                        });
                    }
                });
            } else {
                for (_, sche) in schedules.iter_mut() {
                    sche.check_ticks(now);
                }
                resources.check_ticks(now);
                for table in tables.iter_mut() {
                    table.check_ticks(now);
                }
                for map in maps.iter_mut() {
                    map.check_ticks(now);
                }
            }

            world.trigger::<CheckTicks>(event);
            event
        }

        let this_run = *self.this_run.get_mut();
        let last_check = self.last_check.get();

        if this_run.wrapping_sub(last_check) >= CHECK_CYCLE {
            return Some(check_ticks_cold(self));
        }

        None
    }

    /// Updates internal component trackers.
    ///
    /// Typically called once per frame.
    pub fn clear_trackers(&mut self) {
        self.reset_last_run();
    }
}

// -----------------------------------------------------------------------------
// Count

impl World {
    /// Returns the number of currently alive entities.
    pub fn entity_count(&self) -> usize {
        self.entities.count_spawned()
    }

    /// Returns the number of component types.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Returns the number of resource types.
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Returns the number of schedules stored in this world.
    pub fn schedule_count(&self) -> usize {
        self.schedules.len()
    }
}

// -----------------------------------------------------------------------------
// Error Handling

impl World {
    /// Returns the active default error handler resource.
    ///
    /// Falls back to [`crate::error::panic`] when the resource is absent.
    pub fn fallback_error_handler(&self) -> FallbackErrorHandler {
        self.get_resource::<FallbackErrorHandler>()
            .copied()
            .unwrap_or_default()
    }
}
