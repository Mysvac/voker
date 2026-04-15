use alloc::vec::Vec;

use voker_ecs::borrow::ResMut;
use voker_ecs::derive::Message;
use voker_ecs::prelude::{IntoSystemConfig, MessageReader, MessageWriter};
use voker_ecs::resource::Resource;
use voker_ecs::schedule::{InternedScheduleLabel, SingleThreadedExecutor};
use voker_ecs::schedule::{Schedule, ScheduleLabel};
use voker_ecs::system::Local;
use voker_ecs::world::World;

use crate::{App, Plugin};

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Main;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedMain;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PreStartup;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Startup;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PostStartup;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct SpawnScene;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct First;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PreUpdate;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Update;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct PostUpdate;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Last;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedFirst;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedPreUpdate;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedUpdate;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedPostUpdate;

#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct FixedLast;

#[derive(Message, Debug, Default, Clone, Copy)]
pub struct MainBegin;

#[derive(Message, Debug, Default, Clone, Copy)]
pub struct FixedMainBegin;

#[derive(Resource, Debug)]
pub struct MainScheduleOrder {
    /// The labels to run for the main phase of the [`Main`] schedule (in the order they will be run).
    pub labels: Vec<InternedScheduleLabel>,
    /// The labels to run for the startup phase of the [`Main`] schedule (in the order they will be run).
    pub startup_labels: Vec<InternedScheduleLabel>,
}

#[derive(Resource, Debug)]
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

impl Main {
    /// A system that runs the "main schedule"
    pub fn run_main(world: &mut World, mut no_need_startup: Local<bool>) {
        if !*no_need_startup {
            voker_utils::cold_path();
            world.resource_scope(|world, order: ResMut<MainScheduleOrder>| {
                for &label in &order.startup_labels {
                    world.run_schedule(label);
                }
            });
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

pub struct MainSchedulePlugin;

impl Plugin for MainSchedulePlugin {
    fn build(&self, app: &mut App) {
        let sub = app.main_mut();

        sub.init_schedule(PreStartup)
            .init_schedule(Startup)
            .init_schedule(PostStartup)
            .init_schedule(SpawnScene)
            .init_schedule(First)
            .init_schedule(PreUpdate)
            .init_schedule(Update)
            .init_schedule(PostUpdate)
            .init_schedule(Last)
            .init_schedule(FixedFirst)
            .init_schedule(FixedPreUpdate)
            .init_schedule(FixedUpdate)
            .init_schedule(FixedPostUpdate)
            .init_schedule(FixedLast);

        sub.insert_schedule(Schedule::with_executor(Main, SingleThreadedExecutor::new()));
        sub.insert_schedule(Schedule::with_executor(
            FixedMain,
            SingleThreadedExecutor::new(),
        ));

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
            sched.add_systems(Main::run_main.after(main_begin));
        });

        sub.edit_schedule(FixedMain, |sched| {
            sched.add_systems(FixedMain::run_fixed_main.after(fixed_main_begin));
        });

        fn can_update_messages(
            mut main_reader: MessageReader<MainBegin>,
            mut fixed_main_reader: MessageReader<FixedMainBegin>,
            mut main_ran: Local<bool>,
            mut fixed_main_ran: Local<bool>,
        ) -> bool {
            if !main_reader.is_empty() {
                *main_ran = true;
                main_reader.clear();
            }
            if !fixed_main_reader.is_empty() {
                *fixed_main_ran = true;
                fixed_main_reader.clear();
            }

            if *main_ran && *fixed_main_ran {
                *main_ran = false;
                *fixed_main_ran = false;
                true
            } else {
                false
            }
        }

        sub.edit_schedule(First, |sched| {
            sched.add_systems(World::update_messages.run_if(can_update_messages));
        });
    }
}
