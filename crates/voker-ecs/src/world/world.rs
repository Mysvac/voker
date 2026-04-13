#![expect(clippy::module_inception, reason = "For better structure.")]

use alloc::boxed::Box;
use core::fmt::Debug;
use core::sync::atomic::Ordering;
use voker_os::utils::CachePadded;

use voker_os::sync::atomic::AtomicU32;

use super::WorldId;
use crate::archetype::Archetypes;
use crate::bundle::Bundles;
use crate::command::CommandQueue;
use crate::component::Components;
use crate::entity::{Entities, EntityAllocator};
use crate::error::FallbackErrorHandler;
use crate::message::MessageRegistry;
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
    pub entities: Entities,
    pub allocator: EntityAllocator,
    pub bundles: Bundles,
    pub archetypes: Archetypes,
    pub components: Components,
    pub resources: Resources,
    pub schedules: Schedules,
    pub storages: Storages,
    pub command_queue: CommandQueue,
    pub message_registry: MessageRegistry,
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
            thread_hash: voker_os::thread::thread_hash(),
            entities: Entities::new(),
            allocator: EntityAllocator::new(),
            bundles: Bundles::new(),
            archetypes: Archetypes::new(),
            components: Components::new(),
            resources: Resources::new(),
            schedules: Schedules::new(),
            storages: Storages::new(),
            command_queue: CommandQueue::new(),
            message_registry: MessageRegistry::new(),
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
    pub fn check_ticks(&mut self) -> Option<CheckTicks> {
        let this_run = *self.this_run.get_mut();
        let last_check = self.last_check.get();

        if this_run.wrapping_sub(last_check) >= CHECK_CYCLE {
            voker_utils::cold_path();
            let this_run = Tick::new(this_run);
            let checker = CheckTicks::new(this_run);
            self.storages.check_ticks(checker);
            self.last_check = this_run;
            return Some(checker);
        }

        None
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
// Other

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
