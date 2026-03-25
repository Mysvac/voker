use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Debug;

use voker_ecs::schedule::{InternedScheduleLabel, Schedules};
use voker_ecs::schedule::{Schedule, ScheduleLabel};
use voker_ecs::world::World;
use voker_utils::hash::HashSet;

use crate::{Plugin, PluginsState};

type ExtractFn = Box<dyn FnMut(&mut World, &mut World) + Send>;

pub struct SubApp {
    pub world: Box<World>,
    pub plugin_registry: Vec<Box<dyn Plugin>>,
    pub plugin_names: HashSet<String>,
    pub plugin_build_depth: usize,
    pub plugins_state: PluginsState,
    pub update_schedule: Option<InternedScheduleLabel>,
    pub extract: Option<ExtractFn>,
}

impl Debug for SubApp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SubApp")
    }
}

impl Default for SubApp {
    fn default() -> Self {
        let mut world = World::alloc();
        world.init_resource::<Schedules>();

        Self {
            world,
            plugin_registry: Vec::default(),
            plugin_names: HashSet::default(),
            plugin_build_depth: 0,
            plugins_state: PluginsState::Adding,
            update_schedule: None,
            extract: None,
        }
    }
}

impl SubApp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    // fn run_as_app<F>(&mut self, f: F)
    // where
    //     F: FnOnce(&mut App),
    // {
    //     let mut app = App::empty();
    //     core::mem::swap(self, &mut app.sub_apps.main);
    //     f(&mut app);
    //     core::mem::swap(self, &mut app.sub_apps.main);
    // }

    pub(crate) fn is_building_plugins(&self) -> bool {
        self.plugin_build_depth != 0
    }

    pub fn run_default_schedule(&mut self) {
        if self.is_building_plugins() {
            voker_utils::cold_path();
            panic!("SubApp::update() was called while a plugin was building.");
        }

        if let Some(label) = self.update_schedule {
            self.world.run_schedule(label);
        }
    }

    /// Runs the default schedule and updates internal component trackers.
    pub fn update(&mut self) {
        self.run_default_schedule();
        self.world.update_tick();
    }

    pub fn extract(&mut self, world: &mut World) {
        if let Some(f) = self.extract.as_mut() {
            f(world, &mut self.world);
        }
    }

    pub fn set_extract<F>(&mut self, extract: F) -> &mut Self
    where
        F: FnMut(&mut World, &mut World) + Send + 'static,
    {
        self.extract = Some(Box::new(extract));
        self
    }

    pub fn take_extract(&mut self) -> Option<ExtractFn> {
        self.extract.take()
    }

    pub fn init_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        let label = label.intern();
        let mut schedules = self.world.resource_mut_or_init::<Schedules>();
        if !schedules.contains(label) {
            schedules.insert(Schedule::new(label));
        }
        self
    }

    pub fn schedule(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        let schedules = self.world.resource::<Schedules>()?;
        schedules.get(label)
    }

    pub fn get_schedule_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        let schedules = self.world.resource_mut::<Schedules>()?;
        schedules.into_inner().get_mut(label)
    }

    pub fn edit_schedule(
        &mut self,
        label: impl ScheduleLabel,
        mut f: impl FnMut(&mut Schedule),
    ) -> &mut Self {
        let label = label.intern();
        let mut schedules = self.world.resource_mut_or_init::<Schedules>();

        f(schedules.entry(label));

        self
    }
}
