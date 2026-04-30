use alloc::vec::Vec;

use voker_ecs::borrow::{Res, ResMut};
use voker_ecs::message::{Message, MessageReader, MessageWriter};
use voker_ecs::resource::Resource;
use voker_ecs::schedule::{InternedScheduleLabel, SingleThreadedExecutor};
use voker_ecs::schedule::{
    IntoSystemConfig, IntoSystemSetConfig, Schedule, ScheduleLabel, SystemSet,
};
use voker_ecs::system::Local;
use voker_ecs::world::World;

use crate::{App, DuplicateStrategy, Plugin};

// -----------------------------------------------------------------------------
// Stages

// ---------------------------------------------------------
// Main and FixedMain

/// Root schedule that drives startup and per-frame main phases.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Main;

/// Root schedule for fixed-timestep phases.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedMain;

// ---------------------------------------------------------
// Startup

/// Startup phase that runs before [`Startup`].
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PreStartup;

/// Main startup phase.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Startup;

/// Startup phase that runs after [`Startup`].
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PostStartup;

// ---------------------------------------------------------
// Main

/// Scene spawning phase in the main pipeline.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct SpawnScene;

/// First phase of the per-frame main pipeline.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct First;

/// Pre-update phase of the per-frame main pipeline.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PreUpdate;

/// Runs the [`FixedMain`] schedule in a loop according
/// until all relevant elapsed time has been "consumed".
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct RunFixedMainLoop;

/// Core update phase of the per-frame main pipeline.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Update;

/// Post-update phase of the per-frame main pipeline.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PostUpdate;

/// Final phase of the per-frame main pipeline.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Last;

// ---------------------------------------------------------
// FixedMain

/// First phase of the fixed-timestep pipeline.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedFirst;

/// Pre-update fixed-timestep phase.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedPreUpdate;

/// Core fixed-timestep update phase.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedUpdate;

/// Post-update fixed-timestep phase.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedPostUpdate;

/// Final fixed-timestep phase.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedLast;

// -----------------------------------------------------------------------------
// Message

/// Marker message emitted at the start of each main frame.
#[derive(Message, Debug, Default, Clone, Copy)]
pub struct MainBegin;

/// Marker message emitted at the start of each fixed-timestep frame.
#[derive(Message, Debug, Default, Clone, Copy)]
pub struct FixedMainBegin;

/// A flag indicating that the fixed loop is enabled.
///
/// The `TimePlugin` is required for enabling.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct EnableFixedMain;

// -----------------------------------------------------------------------------
// Order

#[derive(Resource, Debug)]
/// Execution order configuration for [`Main`] startup and per-frame labels.
pub struct MainScheduleOrder {
    /// The labels to run for the main phase of the [`Main`] schedule (in the order they will be run).
    pub labels: Vec<InternedScheduleLabel>,
    /// The labels to run for the startup phase of the [`Main`] schedule (in the order they will be run).
    pub startup_labels: Vec<InternedScheduleLabel>,
}

#[derive(Resource, Debug)]
/// Execution order configuration for [`FixedMain`] labels.
pub struct FixedMainScheduleOrder {
    /// The labels to run for the [`FixedMain`] schedule (in the order they will be run).
    pub labels: Vec<InternedScheduleLabel>,
}

impl Default for MainScheduleOrder {
    fn default() -> Self {
        Self {
            labels: alloc::vec![
                First.intern(),
                PreUpdate.intern(),
                RunFixedMainLoop.intern(),
                Update.intern(),
                SpawnScene.intern(),
                PostUpdate.intern(),
                Last.intern(),
            ],
            startup_labels: alloc::vec![
                PreStartup.intern(),
                Startup.intern(),
                PostStartup.intern(),
            ],
        }
    }
}

impl Default for FixedMainScheduleOrder {
    fn default() -> Self {
        Self {
            labels: alloc::vec![
                FixedFirst.intern(),
                FixedPreUpdate.intern(),
                FixedUpdate.intern(),
                FixedPostUpdate.intern(),
                FixedLast.intern(),
            ],
        }
    }
}

impl MainScheduleOrder {
    /// Adds the given `schedule` after the `after` schedule in the main list of schedules.
    pub fn insert_after(&mut self, after: impl ScheduleLabel, schedule: impl ScheduleLabel) {
        let index = self
            .labels
            .iter()
            .position(|current| (**current).eq(&after))
            .unwrap_or_else(|| panic!("Expected {after:?} to exist"));
        self.labels.insert(index + 1, schedule.intern());
    }

    /// Adds the given `schedule` before the `before` schedule in the main list of schedules.
    pub fn insert_before(&mut self, before: impl ScheduleLabel, schedule: impl ScheduleLabel) {
        let index = self
            .labels
            .iter()
            .position(|current| (**current).eq(&before))
            .unwrap_or_else(|| panic!("Expected {before:?} to exist"));
        self.labels.insert(index, schedule.intern());
    }

    /// Adds the given `schedule` after the `after` schedule in the list of startup schedules.
    pub fn insert_startup_after(
        &mut self,
        after: impl ScheduleLabel,
        schedule: impl ScheduleLabel,
    ) {
        let index = self
            .startup_labels
            .iter()
            .position(|current| (**current).eq(&after))
            .unwrap_or_else(|| panic!("Expected {after:?} to exist"));
        self.startup_labels.insert(index + 1, schedule.intern());
    }

    /// Adds the given `schedule` before the `before` schedule in the list of startup schedules.
    pub fn insert_startup_before(
        &mut self,
        before: impl ScheduleLabel,
        schedule: impl ScheduleLabel,
    ) {
        let index = self
            .startup_labels
            .iter()
            .position(|current| (**current).eq(&before))
            .unwrap_or_else(|| panic!("Expected {before:?} to exist"));
        self.startup_labels.insert(index, schedule.intern());
    }
}

impl FixedMainScheduleOrder {
    /// Adds the given `schedule` after the `after` schedule
    pub fn insert_after(&mut self, after: impl ScheduleLabel, schedule: impl ScheduleLabel) {
        let index = self
            .labels
            .iter()
            .position(|current| (**current).eq(&after))
            .unwrap_or_else(|| panic!("Expected {after:?} to exist"));
        self.labels.insert(index + 1, schedule.intern());
    }

    /// Adds the given `schedule` before the `before` schedule
    pub fn insert_before(&mut self, before: impl ScheduleLabel, schedule: impl ScheduleLabel) {
        let index = self
            .labels
            .iter()
            .position(|current| (**current).eq(&before))
            .unwrap_or_else(|| panic!("Expected {before:?} to exist"));
        self.labels.insert(index, schedule.intern());
    }
}

// -----------------------------------------------------------------------------
// Order

/// Set enum for the systems that want to run inside [`RunFixedMainLoop`],
///
/// but before or after the fixed update logic. Systems in this set will
/// run exactly once per frame, regardless of the number of fixed updates.
/// They will also run under a variable timestep.
#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub enum RunFixedMainLoopSystems {
    BeforeFixedMainLoop,
    FixedMainLoop,
    AfterFixedMainLoop,
}

// -----------------------------------------------------------------------------
// System

impl Main {
    #[cold]
    #[inline(never)]
    fn startup(world: &mut World) {
        world.resource_scope(|world, order: ResMut<MainScheduleOrder>| {
            for &label in &order.startup_labels {
                world.run_schedule(label);
            }
        });
    }

    /// A system that runs the "main schedule"
    pub fn run_main(world: &mut World, mut no_need_startup: Local<bool>) {
        if !*no_need_startup {
            // Separation to reduce function stack frames.
            Main::startup(world);
            *no_need_startup = true;
        }

        world.resource_scope(|world, order: ResMut<MainScheduleOrder>| {
            for &label in &order.labels {
                world.run_schedule(label);
            }
        });
    }
}

impl FixedMain {
    /// A system that runs the fixed timestep's "main schedule"
    pub fn run_fixed_main(world: &mut World) {
        world.resource_scope(|world, order: ResMut<FixedMainScheduleOrder>| {
            for &label in &order.labels {
                world.run_schedule(label);
            }
        });
    }
}

// -----------------------------------------------------------------------------
// MainSchedulePlugin

/// Built-in scheduler plugin automatically added by [`App::new`](crate::App::new).
///
/// Default order:
///
/// **Main-Startup**: (run_once)
///
/// 1. [`PreStartup`]
/// 2. [`Startup`]
/// 3. [`PostStartup`]
///
/// **Main-Loop**:
///
/// 1. [`First`]
/// 2. [`PreUpdate`]
/// 3. [`RunFixedMainLoop`]
///    -> run **FixedMain-Loop**
/// 4. [`Update`]
/// 5. [`SpawnScene`]
/// 6. [`PostUpdate`]
/// 7. [`Last`]
///
/// **FixedMain-Loop**:
///
/// 1. [`FixedFirst`]
/// 2. [`FixedPreUpdate`]
/// 3. [`FixedUpdate`]
/// 4. [`FixedPostUpdate`]
/// 5. [`FixedLast`]
///
/// This plugin also wires periodic message queue maintenance by running
/// [`World::update_messages`] in [`First`] when both main and fixed phases have started.
pub struct MainSchedulePlugin;

impl Plugin for MainSchedulePlugin {
    fn build(&self, app: &mut App) {
        let sub = app.main_mut();

        sub.set_main_schedule(Main);

        sub.init_schedule(PreStartup)
            .init_schedule(Startup)
            .init_schedule(PostStartup)
            .init_schedule(SpawnScene)
            .init_schedule(First)
            .init_schedule(PreUpdate)
            .init_schedule(RunFixedMainLoop)
            .init_schedule(Update)
            .init_schedule(PostUpdate)
            .init_schedule(Last)
            .init_schedule(FixedFirst)
            .init_schedule(FixedPreUpdate)
            .init_schedule(FixedUpdate)
            .init_schedule(FixedPostUpdate)
            .init_schedule(FixedLast);

        // For linear tasks, single-threaded executor is faster.
        let exec = SingleThreadedExecutor::new();
        sub.insert_schedule(Schedule::with_executor(Main, exec));
        let exec = SingleThreadedExecutor::new();
        sub.insert_schedule(Schedule::with_executor(FixedMain, exec));

        sub.init_resource::<MainScheduleOrder>();
        sub.init_resource::<FixedMainScheduleOrder>();

        sub.add_message::<MainBegin>();
        sub.add_message::<FixedMainBegin>();

        fn main_begin(mut writer: MessageWriter<MainBegin>) {
            writer.write_default();
        }

        fn fixed_main_begin(mut writer: MessageWriter<FixedMainBegin>) {
            writer.write_default();
        }

        sub.edit_schedule(Main, |sched| {
            sched.add_systems((), Main::run_main.after(main_begin));
        });

        sub.edit_schedule(FixedMain, |sched| {
            sched.add_systems((), FixedMain::run_fixed_main.after(fixed_main_begin));
        });

        sub.edit_schedule(First, |sched| {
            sched.add_systems((), World::update_messages.run_if(message_update_condition));
        });

        sub.edit_schedule(RunFixedMainLoop, |sched| {
            use RunFixedMainLoopSystems::*;
            sched.config_set(BeforeFixedMainLoop.run_before(FixedMainLoop));
            sched.config_set(FixedMainLoop.run_before(AfterFixedMainLoop));
        });
    }

    fn duplicate_strategy(&self) -> DuplicateStrategy {
        // The main scheduler is internal plugin that is automatically
        // added during App::new, should not be added repeatedly.
        DuplicateStrategy::Panic
    }
}

fn message_update_condition(
    mut main_reader: MessageReader<MainBegin>,
    mut fixed_main_reader: MessageReader<FixedMainBegin>,
    mut main_ran: Local<bool>,
    mut fixed_main_ran: Local<bool>,
    enable_fixed: Option<Res<EnableFixedMain>>,
) -> bool {
    if !main_reader.is_empty() {
        *main_ran = true;
        main_reader.clear();
    }

    if enable_fixed.is_some() {
        if !fixed_main_reader.is_empty() {
            *fixed_main_ran = true;
            fixed_main_reader.clear();
        }
        if *main_ran && *fixed_main_ran {
            *main_ran = false;
            *fixed_main_ran = false;
            return true;
        }
    } else {
        if *main_ran {
            *main_ran = false;
            return true;
        }
    }
    false
}
