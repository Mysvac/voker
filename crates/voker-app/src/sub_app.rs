use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use core::fmt::Debug;
use voker_ecs::prelude::{IntoObserver, Message};
use voker_ecs::reflect::AppTypeRegistry;
use voker_reflect::info::Typed;

use voker_ecs::resource::Resource;
use voker_ecs::schedule::{InternedScheduleLabel, IntoSystemConfig};
use voker_ecs::schedule::{Schedule, ScheduleLabel};
use voker_ecs::system::IntoSystem;
use voker_ecs::world::{FromWorld, World};
use voker_reflect::registry::{FromType, GetTypeMeta, TypeData};
use voker_reflect::{Reflect, info::TypePath};
use voker_utils::hash::HashSet;

use crate::plugin::PlaceholderPlugin;
use crate::{App, Plugin, Plugins, PluginsState};

type ExtractFn = Box<dyn FnMut(&mut World, &mut World) + Send>;

// -----------------------------------------------------------------------------
// SubApp

/// A secondary app container with an isolated [`World`].
///
/// Sub-apps are typically used for specialized pipelines that need their own
/// schedules and resources, while still being orchestrated by the main [`App`].
pub struct SubApp {
    pub(crate) world: Option<Box<World>>,
    pub(crate) plugins: Vec<Box<dyn Plugin>>,
    pub(crate) plugin_names: HashSet<&'static str>,
    pub(crate) plugin_build_depth: usize,
    pub(crate) plugins_state: PluginsState,
    pub(crate) main_schedule: Option<InternedScheduleLabel>,
    pub(crate) extract: Option<ExtractFn>,
}

impl Debug for SubApp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("SubApp")
    }
}

impl Default for SubApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SubApp {
    /// Creates an empty sub-app without allocating a world.
    ///
    /// This is primarily used internally as a placeholder for temporary swaps.
    pub const fn empty() -> Self {
        Self {
            world: None,
            plugins: Vec::new(),
            plugin_names: HashSet::new(),
            plugin_build_depth: 0,
            plugins_state: PluginsState::Adding,
            main_schedule: None,
            extract: None,
        }
    }

    /// Creates a sub-app with a newly allocated world.
    pub fn new() -> Self {
        Self {
            world: Some(World::alloc()),
            plugins: Vec::default(),
            plugin_names: HashSet::default(),
            plugin_build_depth: 0,
            plugins_state: PluginsState::Adding,
            main_schedule: None,
            extract: None,
        }
    }

    /// Returns an immutable reference to this sub-app world.
    pub fn world(&self) -> &World {
        self.world.as_ref().unwrap()
    }

    /// Returns a mutable reference to this sub-app world.
    pub fn world_mut(&mut self) -> &mut World {
        self.world.as_mut().unwrap()
    }

    /// Returns the schedule label executed by [`Self::run_main_schedule`], if any.
    pub fn main_schedule(&mut self) -> Option<InternedScheduleLabel> {
        self.main_schedule // copyable
    }

    /// Runs this sub-app's configured main schedule.
    ///
    /// If no main schedule is configured, this is a no-op.
    pub fn run_main_schedule(&mut self) {
        if self.plugin_build_depth != 0 {
            voker_utils::cold_path();
            panic!("SubApp::update() was called while a plugin was building.");
        }

        let world = self.world.as_mut().unwrap();

        if let Some(label) = self.main_schedule {
            world.run_schedule(label);
        }
    }

    /// Runs the configured main schedule and clears world trackers.
    pub fn update(&mut self) {
        self.run_main_schedule();
        self.world_mut().clear_trackers();
    }

    /// Runs the configured extract function, if present.
    ///
    /// The first argument to the extract callback is usually the main world,
    /// and the second argument is this sub-app world.
    pub fn extract(&mut self, world: &mut World) {
        let this = self.world.as_mut().unwrap();
        if let Some(f) = self.extract.as_mut() {
            f(world, this);
        }
    }

    fn run_as_app(&mut self, f: impl FnOnce(&mut App)) {
        let mut app = App::empty();
        core::mem::swap(&mut app.main, self);
        f(&mut app);
        core::mem::swap(&mut app.main, self);
    }

    /// Sets the schedule run by [`Self::run_main_schedule`].
    pub fn set_main_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        self.main_schedule = Some(label.intern());
        self
    }

    /// Sets the extract callback used before sub-app updates.
    pub fn set_extract<F>(&mut self, extract: F) -> &mut Self
    where
        F: FnMut(&mut World, &mut World) + Send + 'static,
    {
        self.extract = Some(Box::new(extract));
        self
    }

    /// Registers reflected type metadata for `T` in [`AppTypeRegistry`].
    pub fn register_type<T: GetTypeMeta>(&mut self) -> &mut Self {
        let registry = self.world().resource::<AppTypeRegistry>();
        registry.write().register::<T>();
        self
    }

    /// Registers type data `D` for reflected type `T` in [`AppTypeRegistry`].
    pub fn register_type_data<T: Typed, D: TypeData + FromType<T>>(&mut self) -> &mut Self {
        let registry = self.world().resource::<AppTypeRegistry>();
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
        let registry = self.world().resource::<AppTypeRegistry>();
        registry.write().register_type_conversion::<T, U, F>(function);
        self
    }

    /// Registers an infallible `Into` conversion route from `T` to `U` in [`AppTypeRegistry`].
    pub fn register_into_type_conversion<T, U>(&mut self) -> &mut Self
    where
        T: Reflect + TypePath,
        U: Reflect + TypePath + From<T>,
    {
        let registry = self.world().resource::<AppTypeRegistry>();
        registry.write().register_into_type_conversion::<T, U>();
        self
    }

    /// Initializes a send resource if missing in this sub-app world.
    pub fn init_resource<R: Resource + Send + FromWorld>(&mut self) -> &mut Self {
        self.world_mut().init_resource::<R>();
        self
    }

    /// Initializes a non-send resource if missing in this sub-app world.
    pub fn init_non_send<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.world_mut().init_non_send::<R>();
        self
    }

    /// Inserts or replaces a send resource in this sub-app world.
    pub fn insert_resource<R: Resource + Send>(&mut self, resource: R) -> &mut Self {
        self.world_mut().insert_resource(resource);
        self
    }

    /// Inserts or replaces a non-send resource in this sub-app world.
    pub fn insert_non_send<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world_mut().insert_non_send(resource);
        self
    }

    /// Ensures a schedule with the given label exists.
    pub fn init_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        self.world_mut().schedule_entry(label.intern());
        self
    }

    /// Inserts or replaces a schedule in this sub-app world.
    pub fn insert_schedule(&mut self, schedule: Schedule) -> &mut Self {
        self.world_mut().insert_schedule(schedule);
        self
    }

    /// Returns a mutable reference to the schedule associated with label, if it exists.
    pub fn get_schedule_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        self.world_mut().schedules.get_mut(label.intern())
    }

    /// Returns a reference to the schedule associated with label, if it exists.
    pub fn get_schedule(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        self.world().schedules.get(label.intern())
    }

    /// Edits a schedule in place, creating it if necessary.
    pub fn edit_schedule(
        &mut self,
        label: impl ScheduleLabel,
        mut f: impl FnMut(&mut Schedule),
    ) -> &mut Self {
        f(self.world_mut().schedule_entry(label.intern()));
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
        self.world_mut().add_system(label.intern(), system);
        self
    }

    /// Adds systems/configuration to the given schedule label.
    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        systems: impl IntoSystemConfig<M>,
    ) -> &mut Self {
        self.world_mut().add_systems(label.intern(), systems);
        self
    }

    /// Adds a global observer to this sub-app world.
    pub fn add_observer<M>(&mut self, observer: impl IntoObserver<M>) -> &mut Self {
        self.world_mut().add_observer(observer);
        self
    }

    /// Registers a message type in this sub-app world.
    pub fn add_message<T: Message>(&mut self) -> &mut Self {
        self.world_mut().register_message::<T>();
        self
    }

    /// Adds plugins or plugin groups to this sub-app.
    pub fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        self.run_as_app(|app| plugins.add_to_app(app));
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
    pub fn get_added_plugin<T>(&self) -> Option<&T>
    where
        T: Plugin,
    {
        for p in self.plugins.iter() {
            if let Some(ret) = (p.as_ref() as &dyn Any).downcast_ref::<T>() {
                return Some(ret);
            }
        }
        None
    }

    /// Returns the current plugin lifecycle state for this sub-app.
    pub fn plugins_state(&mut self) -> PluginsState {
        if self.plugins_state == PluginsState::Adding {
            let plugins = core::mem::take(&mut self.plugins);
            let mut state = PluginsState::Ready;
            // Plugins are usually not empty
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
        }

        self.plugins_state
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

    /// Returns `true` if this sub-app is currently within plugin build execution.
    pub fn is_building_plugins(&self) -> bool {
        self.plugin_build_depth != 0
    }
}
