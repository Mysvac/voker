use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use core::fmt::Debug;
use voker_ecs::prelude::{IntoObserver, Message};
use voker_ecs::reflect::AppTypeRegistry;

use voker_ecs::resource::Resource;
use voker_ecs::schedule::{InternedScheduleLabel, IntoSystemConfig};
use voker_ecs::schedule::{Schedule, ScheduleLabel};
use voker_ecs::system::{IntoSystem, SystemInput};
use voker_ecs::world::{FromWorld, World};
use voker_reflect::registry::{FromType, GetTypeMeta, TypeData};
use voker_reflect::{Reflect, info::TypePath};
use voker_utils::hash::HashSet;

use crate::plugin::PlaceholderPlugin;
use crate::{App, Plugin, Plugins, PluginsState};

type ExtractFn = Box<dyn FnMut(&mut World, &mut World) + Send>;

// -----------------------------------------------------------------------------
// SubApp

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
    /// Creates a new sub-app with default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an immutable reference to this sub-app world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Returns a mutable reference to this sub-app world.
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

    /// Runs this sub-app's configured default schedule, if present.
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
        self.world.clear_trackers();
    }

    /// Runs the configured extract callback, if any.
    pub fn extract(&mut self, world: &mut World) {
        if let Some(f) = self.extract.as_mut() {
            f(world, &mut self.world);
        }
    }

    /// Sets the extract callback used before sub-app updates.
    pub fn set_extract<F>(&mut self, extract: F) -> &mut Self
    where
        F: FnMut(&mut World, &mut World) + Send + 'static,
    {
        self.extract = Some(Box::new(extract));
        self
    }

    /// Removes and returns the current extract callback.
    pub fn take_extract(&mut self) -> Option<ExtractFn> {
        self.extract.take()
    }

    /// Initializes a send resource if missing in this sub-app world.
    pub fn init_resource<R: Resource + Send + FromWorld>(&mut self) -> &mut Self {
        self.world.init_resource::<R>();
        self
    }

    /// Initializes a non-send resource if missing in this sub-app world.
    pub fn init_non_send<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.world.init_non_send::<R>();
        self
    }

    /// Inserts or replaces a send resource in this sub-app world.
    pub fn insert_resource<R: Resource + Send>(&mut self, resource: R) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    /// Inserts or replaces a non-send resource in this sub-app world.
    pub fn insert_non_send<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world.insert_non_send(resource);
        self
    }

    /// Adds one system to the given schedule label.
    ///
    /// This function is faster then `add_systems`.
    pub fn add_system<M>(
        &mut self,
        label: impl ScheduleLabel,
        system: impl IntoSystem<(), (), M>,
    ) -> &mut Self {
        self.world.add_system(label, system);
        self
    }

    /// Adds systems/configuration to the given schedule label.
    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        systems: impl IntoSystemConfig<M>,
    ) -> &mut Self {
        self.world.add_systems(label, systems);
        self
    }

    /// Registers a system in this sub-app world.
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

    /// Ensures a schedule with the given label exists.
    pub fn init_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        let label = label.intern();
        let schedules = &mut self.world.schedules;
        schedules.entry(label);
        self
    }

    /// Inserts or replaces a schedule in this sub-app world.
    pub fn insert_schedule(&mut self, schedule: Schedule) -> &mut Self {
        self.world.insert_schedule(schedule);
        self
    }

    /// Returns an immutable schedule by label.
    pub fn get_schedule(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        let schedules = &self.world.schedules;
        schedules.get(label)
    }

    /// Returns a mutable schedule by label.
    pub fn get_schedule_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        let schedules = &mut self.world.schedules;
        schedules.get_mut(label)
    }

    /// Edits a schedule in place, creating it if necessary.
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

    /// Adds a global observer to this sub-app world.
    pub fn add_observer<M>(&mut self, observer: impl IntoObserver<M>) -> &mut Self {
        self.world.add_observer(observer);
        self
    }

    /// Registers a message type in this sub-app world.
    pub fn add_message<T>(&mut self) -> &mut Self
    where
        T: Message,
    {
        self.world.register_message::<T>();
        self
    }

    /// Registers reflected type metadata for `T` in [`AppTypeRegistry`].
    pub fn register_type<T: GetTypeMeta>(&mut self) -> &mut Self {
        let registry = self.world.resource_mut::<AppTypeRegistry>();
        registry.write().register::<T>();
        self
    }

    /// Registers type data `D` for reflected type `T` in [`AppTypeRegistry`].
    pub fn register_type_data<T: voker_reflect::info::Typed, D: TypeData + FromType<T>>(
        &mut self,
    ) -> &mut Self {
        let registry = self.world.resource_mut::<AppTypeRegistry>();
        registry.write().register_type_data::<T, D>();
        self
    }

    /// Registers a fallible conversion route from `T` to `U` in [`AppTypeRegistry`].
    pub fn register_type_conversion<T, U, F>(&mut self, function: F) -> &mut Self
    where
        T: Reflect + TypePath,
        U: Reflect + TypePath,
        F: Fn(T) -> Result<U, T> + Clone + Send + Sync + 'static,
    {
        let registry = self.world.resource_mut::<AppTypeRegistry>();
        registry.write().register_type_conversion::<T, U, F>(function);
        self
    }

    /// Registers an infallible `Into` conversion route from `T` to `U` in [`AppTypeRegistry`].
    pub fn register_into_type_conversion<T, U>(&mut self) -> &mut Self
    where
        T: Reflect + TypePath,
        U: Reflect + TypePath + From<T>,
    {
        let registry = self.world.resource_mut::<AppTypeRegistry>();
        registry.write().register_into_type_conversion::<T, U>();
        self
    }

    /// Adds plugins or plugin groups to this sub-app.
    pub fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        self.run_as_app(|app| {
            plugins.add_to_app(app);
        });
        self
    }

    /// Returns `true` if a plugin of type `T` has been added.
    pub fn is_plugin_added<T>(&self) -> bool
    where
        T: Plugin,
    {
        self.plugin_names.contains(core::any::type_name::<T>())
    }

    /// Returns all added plugin instances matching type `T`.
    pub fn get_added_plugins<T>(&self) -> Vec<&T>
    where
        T: Plugin,
    {
        self.plugins
            .iter()
            .filter_map(|p| (p.as_ref() as &dyn Any).downcast_ref::<T>())
            .collect()
    }

    /// Returns the current plugin lifecycle state for this sub-app.
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

    /// Calls [`Plugin::finish`] on all plugins in this sub-app.
    pub fn finish(&mut self) {
        let mut placeholder: Box<dyn Plugin> = Box::new(PlaceholderPlugin);
        for i in 0..self.plugins.len() {
            core::mem::swap(&mut self.plugins[i], &mut placeholder);
            self.run_as_app(|app| {
                placeholder.finish(app);
            });
            core::mem::swap(&mut self.plugins[i], &mut placeholder);
        }
        self.plugins_state = PluginsState::Finished;
    }

    /// Calls [`Plugin::cleanup`] on all plugins in this sub-app.
    pub fn cleanup(&mut self) {
        let mut placeholder: Box<dyn Plugin> = Box::new(PlaceholderPlugin);
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
