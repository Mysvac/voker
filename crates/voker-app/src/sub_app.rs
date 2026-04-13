use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Debug;
use voker_ecs::prelude::Message;

use voker_ecs::resource::Resource;
use voker_ecs::schedule::{InternedScheduleLabel, IntoSystemConfig};
use voker_ecs::schedule::{Schedule, ScheduleLabel};
use voker_ecs::system::{IntoSystem, SystemError, SystemInput};
use voker_ecs::world::World;
use voker_utils::hash::HashSet;

use crate::{App, Plugin, Plugins, PluginsState};

type ExtractFn = Box<dyn FnMut(&mut World, &mut World) + Send>;

pub struct SubApp {
    pub(crate) world: Box<World>,
    pub(crate) plugins: Vec<Box<dyn Plugin>>,
    pub(crate) plugin_names: HashSet<String>,
    pub(crate) plugin_build_depth: usize,
    pub(crate) plugins_state: PluginsState,
    pub(crate) update_schedule: Option<InternedScheduleLabel>,
    pub(crate) extract: Option<ExtractFn>,
}

impl Debug for SubApp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SubApp")
    }
}

impl Default for SubApp {
    fn default() -> Self {
        Self {
            world: World::alloc(),
            plugins: Vec::default(),
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

    fn run_as_app<F>(&mut self, f: F)
    where
        F: FnOnce(&mut App),
    {
        let mut app = App::empty();
        core::mem::swap(self, &mut app.sub_apps.main);
        f(&mut app);
        core::mem::swap(self, &mut app.sub_apps.main);
    }

    pub fn run_default_schedule(&mut self) {
        if self.plugin_build_depth != 0 {
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
        self.world.clear_tracker();
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

    pub fn insert_resource<R: Resource + Send>(&mut self, resource: R) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn init_resource<R: Resource + Send + voker_ecs::world::FromWorld>(&mut self) -> &mut Self {
        self.world.init_resource::<R>();
        self
    }

    pub fn insert_non_send<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world.insert_non_send(resource);
        self
    }

    pub fn init_non_send<R: Resource + voker_ecs::world::FromWorld>(&mut self) -> &mut Self {
        self.world.init_non_send::<R>();
        self
    }

    pub fn init_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        let label = label.intern();
        let schedules = &mut self.world.schedules;
        schedules.entry(label);
        self
    }

    pub fn insert_schedule(&mut self, schedule: Schedule) -> Option<Schedule> {
        self.world.insert_schedule(schedule)
    }

    pub fn remove_schedule(&mut self, label: impl ScheduleLabel) -> Option<Schedule> {
        self.world.remove_schedule(label)
    }

    pub fn get_schedule(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        let schedules = &self.world.schedules;
        schedules.get(label)
    }

    pub fn get_schedule_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        let schedules = &mut self.world.schedules;
        schedules.get_mut(label)
    }

    pub fn edit_schedule(
        &mut self,
        label: impl ScheduleLabel,
        mut f: impl FnMut(&mut Schedule),
    ) -> &mut Self {
        let label = label.intern();
        let schedules = &mut self.world.schedules;

        f(schedules.entry(label));

        self
    }

    pub fn run_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        self.world.run_schedule(label);
        self
    }

    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        systems: impl IntoSystemConfig<M>,
    ) -> &mut Self {
        let label = label.intern();
        self.world.schedules.entry(label).add_systems(systems);
        self
    }

    pub fn register_system<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + 'static,
    ) -> &mut Self
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        self.world.register_system(system);
        self
    }

    pub fn run_system_with<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + 'static,
        input: I::Data<'_>,
    ) -> Result<O, SystemError>
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        self.world.run_system_with(system, input)
    }

    pub fn run_system<O: 'static, M>(
        &mut self,
        system: impl IntoSystem<(), O, M> + 'static,
    ) -> Result<O, SystemError> {
        self.world.run_system(system)
    }

    pub fn add_message<T>(&mut self) -> &mut Self
    where
        T: Message,
    {
        self.world.register_message::<T>();
        self
    }

    pub fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        self.run_as_app(|app| {
            plugins.add_to_app(app);
        });
        self
    }

    pub fn is_plugin_added<T>(&self) -> bool
    where
        T: Plugin,
    {
        self.plugin_names.contains(core::any::type_name::<T>())
    }

    pub fn get_added_plugins<T>(&self) -> Vec<&T>
    where
        T: Plugin,
    {
        self.plugins
            .iter()
            .filter_map(|p| (p.as_ref() as &dyn core::any::Any).downcast_ref::<T>())
            .collect()
    }

    pub fn plugins_state(&mut self) -> PluginsState {
        match self.plugins_state {
            PluginsState::Adding => {
                let mut state = PluginsState::Ready;
                let plugins = core::mem::take(&mut self.plugins);
                self.run_as_app(|app| {
                    for plugin in &plugins {
                        if !plugin.ready(app) {
                            state = PluginsState::Adding;
                            break;
                        }
                    }
                });
                self.plugins = plugins;
                self.plugins_state = state;
                state
            }
            state => state,
        }
    }

    pub fn finish(&mut self) {
        let mut placeholder: Box<dyn Plugin> = Box::new(Placeholder);
        for i in 0..self.plugins.len() {
            core::mem::swap(&mut self.plugins[i], &mut placeholder);
            self.run_as_app(|app| {
                placeholder.finish(app);
            });
            core::mem::swap(&mut self.plugins[i], &mut placeholder);
        }
        self.plugins_state = PluginsState::Finished;
    }

    pub fn cleanup(&mut self) {
        let mut placeholder: Box<dyn Plugin> = Box::new(Placeholder);
        for i in 0..self.plugins.len() {
            core::mem::swap(&mut self.plugins[i], &mut placeholder);
            self.run_as_app(|app| {
                placeholder.cleanup(app);
            });
            core::mem::swap(&mut self.plugins[i], &mut placeholder);
        }
        self.plugins_state = PluginsState::Cleaned;
    }

    pub(crate) fn is_building_plugins(&self) -> bool {
        self.plugin_build_depth != 0
    }
}

struct Placeholder;

impl Plugin for Placeholder {
    fn build(&self, _app: &mut App) {}
}
