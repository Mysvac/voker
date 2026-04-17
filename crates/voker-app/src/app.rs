use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::panic::AssertUnwindSafe;
use voker_ecs::message::{MessageCursor, MessageQueue};
use voker_ecs::prelude::{Component, IntoObserver, Message};
use voker_ecs::reflect::AppTypeRegistry;
use voker_ecs::world::{FromWorld, World};

use voker_ecs::error::ErrorHandler;
use voker_ecs::error::FallbackErrorHandler;
use voker_ecs::resource::Resource;
use voker_ecs::schedule::Schedule;
use voker_ecs::schedule::{IntoSystemConfig, ScheduleLabel};
use voker_ecs::system::{IntoSystem, SystemInput};
use voker_reflect::registry::{FromType, GetTypeMeta, TypeData};
use voker_reflect::{Reflect, info::TypePath};
use voker_utils::hash::HashMap;

use crate::InternedAppLabel;
use crate::MainSchedulePlugin;
use crate::Plugin;
use crate::Plugins;
use crate::PluginsState;
use crate::SubApp;
use crate::main_schedule::Main;
use crate::plugin::PlaceholderPlugin;
use crate::{AppExit, AppLabel};

type RunnerFn = Box<dyn FnOnce(App) -> AppExit>;

/// [`App`] is the primary API for writing user applications.
///
/// It automates the setup of a [standard lifecycle](Main) and
/// provides interface glue for [plugins](`Plugin`).
///
/// A single [`App`] can contain multiple [`SubApp`] instances,
/// but [`App`] methods only affect the "main" one. To access a
/// particular [`SubApp`], use [`get_sub_app`](App::get_sub_app)
/// or [`get_sub_app_mut`](App::get_sub_app_mut).
///
/// # Examples
///
/// Here is a simple "Hello World" voker app:
///
/// ```
/// # use voker_app::{App, Update};
/// # use voker_ecs::prelude::*;
/// #
/// fn main() {
///    App::new()
///        .add_systems(Update, hello_world_system)
///        .run();
/// }
///
/// fn hello_world_system() {
///    println!("hello world");
/// }
/// ```
#[must_use]
pub struct App {
    pub(crate) sub_apps: SubApps,
    pub(crate) runner: RunnerFn,
    pub(crate) error_handler: Option<ErrorHandler>,
}

#[derive(Default)]
pub struct SubApps {
    /// The primary sub-app that contains the "main" world.
    pub main: SubApp,
    pub sub_apps: HashMap<InternedAppLabel, SubApp>,
}

impl Debug for App {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "App {{ sub_apps: ")?;
        f.debug_map().entries(self.sub_apps.sub_apps.iter()).finish()?;
        write!(f, "}}")
    }
}

impl Default for App {
    fn default() -> Self {
        let mut app = App::empty();
        app.sub_apps.main.update_schedule = Some(Main.intern());
        app.add_plugins(MainSchedulePlugin);
        app.add_message::<AppExit>();

        let mut registry = AppTypeRegistry::default();
        registry.auto_register();
        app.insert_resource(registry);

        app
    }
}

impl App {
    /// Creates a new app with the default setup.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty app without default plugins or schedules.
    pub fn empty() -> Self {
        Self {
            sub_apps: SubApps {
                main: SubApp::new(),
                sub_apps: HashMap::default(),
            },
            runner: Box::new(run_once),
            error_handler: None,
        }
    }

    /// Runs one update step for this app and all registered sub-apps.
    pub fn update(&mut self) {
        if self.is_building_plugins() {
            voker_utils::cold_path();
            panic!("App::update() was called while a plugin was building.");
        }
        self.sub_apps.update();
    }

    /// Runs the app by consuming the configured runner.
    pub fn run(&mut self) -> AppExit {
        if self.is_building_plugins() {
            voker_utils::cold_path();
            panic!("App::run() was called while a plugin was building.");
        }

        let runner = core::mem::replace(&mut self.runner, Box::new(run_once));
        let app = core::mem::replace(self, App::empty());
        (runner)(app)
    }

    /// Sets the function used when [`App::run`] is called.
    pub fn set_runner(&mut self, runner: impl FnOnce(App) -> AppExit + 'static) -> &mut Self {
        self.runner = Box::new(runner);
        self
    }

    /// Returns the aggregate plugin state across main and sub-apps.
    pub fn plugins_state(&mut self) -> PluginsState {
        let mut overall = self.main_mut().plugins_state();
        self.sub_apps
            .sub_apps
            .values_mut()
            .for_each(|app| overall = overall.min(app.plugins_state()));
        overall
    }

    /// Returns all sub-app containers.
    pub fn sub_apps(&self) -> &SubApps {
        &self.sub_apps
    }

    /// Returns mutable access to all sub-app containers.
    pub fn sub_apps_mut(&mut self) -> &mut SubApps {
        &mut self.sub_apps
    }

    /// Returns a reference to the [`SubApp`] with the given label.
    ///
    /// # Panics
    ///
    /// Panics if the [`SubApp`] doesn't exist.
    pub fn sub_app(&self, label: impl AppLabel) -> &SubApp {
        let str = label.intern();
        self.get_sub_app(label).unwrap_or_else(|| {
            panic!("No sub-app with label '{:?}' exists.", str);
        })
    }

    /// Returns a reference to the [`SubApp`] with the given label.
    ///
    /// # Panics
    ///
    /// Panics if the [`SubApp`] doesn't exist.
    pub fn sub_app_mut(&mut self, label: impl AppLabel) -> &mut SubApp {
        let str = label.intern();
        self.get_sub_app_mut(label).unwrap_or_else(|| {
            panic!("No sub-app with label '{:?}' exists.", str);
        })
    }

    /// Returns the sub-app for `label`, if it exists.
    pub fn get_sub_app(&self, label: impl AppLabel) -> Option<&SubApp> {
        self.sub_apps.sub_apps.get(&label.intern())
    }

    /// Returns mutable access to the sub-app for `label`, if it exists.
    pub fn get_sub_app_mut(&mut self, label: impl AppLabel) -> Option<&mut SubApp> {
        self.sub_apps.sub_apps.get_mut(&label.intern())
    }

    /// Inserts or replaces a sub-app at `label`.
    pub fn insert_sub_app(&mut self, label: impl AppLabel, mut app: SubApp) -> &mut Self {
        if let Some(handler) = self.error_handler
            && app.world_mut().get_resource::<FallbackErrorHandler>().is_none()
        {
            app.world_mut().insert_resource(FallbackErrorHandler(handler));
        }

        let _old = self.sub_apps.sub_apps.insert(label.intern(), app);
        self
    }

    /// Removes and returns the sub-app at `label`, if present.
    pub fn remove_sub_app(&mut self, label: impl AppLabel) -> Option<SubApp> {
        self.sub_apps.sub_apps.remove(&label.intern())
    }

    /// Returns the main sub-app.
    pub fn main(&self) -> &SubApp {
        &self.sub_apps.main
    }

    /// Returns mutable access to the main sub-app.
    pub fn main_mut(&mut self) -> &mut SubApp {
        &mut self.sub_apps.main
    }

    /// Returns the world of the main sub-app.
    pub fn world(&self) -> &World {
        self.main().world()
    }

    /// Returns mutable access to the world of the main sub-app.
    pub fn world_mut(&mut self) -> &mut World {
        self.main_mut().world_mut()
    }

    /// Calls [`Plugin::finish`] on all loaded plugins.
    pub fn finish(&mut self) {
        let mut placeholder: Box<dyn Plugin> = Box::new(PlaceholderPlugin);
        for i in 0..self.main().plugins.len() {
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
            placeholder.finish(self);
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
        }
        self.main_mut().plugins_state = PluginsState::Finished;

        self.sub_apps.sub_apps.values_mut().for_each(SubApp::finish);
    }

    /// Calls [`Plugin::cleanup`] on all loaded plugins.
    pub fn cleanup(&mut self) {
        let mut placeholder: Box<dyn Plugin> = Box::new(PlaceholderPlugin);
        for i in 0..self.main().plugins.len() {
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
            placeholder.cleanup(self);
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
        }
        self.main_mut().plugins_state = PluginsState::Cleaned;

        self.sub_apps.sub_apps.values_mut().for_each(SubApp::cleanup);
    }

    pub(crate) fn is_building_plugins(&self) -> bool {
        self.sub_apps.iter().any(SubApp::is_building_plugins)
    }

    /// Adds one system to a schedule in the main sub-app.
    ///
    /// This function is faster then `add_systems`.
    pub fn add_system<M>(
        &mut self,
        label: impl ScheduleLabel,
        system: impl IntoSystem<(), (), M>,
    ) -> &mut Self {
        self.main_mut().add_system(label, system);
        self
    }

    /// Adds systems/configuration to a schedule in the main sub-app.
    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        systems: impl IntoSystemConfig<M>,
    ) -> &mut Self {
        self.main_mut().add_systems(label, systems);
        self
    }

    /// Registers a system in the main world and returns `self`.
    pub fn register_system<I, O, M>(
        &mut self,
        system: impl IntoSystem<I, O, M> + 'static,
    ) -> &mut Self
    where
        I: SystemInput + 'static,
        O: 'static,
    {
        self.main_mut().register_system(system);
        self
    }

    /// Registers a message type in the main world.
    pub fn add_message<M: Message>(&mut self) -> &mut Self {
        self.main_mut().add_message::<M>();
        self
    }

    /// Initializes a send resource if it is missing.
    pub fn init_resource<R: Resource + Send + FromWorld>(&mut self) -> &mut Self {
        self.main_mut().init_resource::<R>();
        self
    }

    /// Inserts or replaces a send resource in the main world.
    pub fn insert_resource<R: Resource + Send>(&mut self, resource: R) -> &mut Self {
        self.main_mut().insert_resource(resource);
        self
    }

    /// Inserts or replaces a non-send resource in the main world.
    pub fn insert_non_send<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.main_mut().insert_non_send(resource);
        self
    }

    /// Initializes a non-send resource if it is missing.
    pub fn init_non_send<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.main_mut().init_non_send::<R>();
        self
    }

    /// Adds plugins or plugin groups to this app.
    #[track_caller]
    pub fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        if matches!(
            self.plugins_state(),
            PluginsState::Finished | PluginsState::Cleaned
        ) {
            voker_utils::cold_path();
            panic!("Plugins cannot be added after App::finish() or App::cleanup().");
        }
        plugins.add_to_app(self);
        self
    }

    pub(crate) fn add_boxed_plugin(
        &mut self,
        plugin: Box<dyn Plugin>,
    ) -> Result<(), Box<dyn Plugin>> {
        let plugin_name = plugin.name().to_string();
        log::debug!("added plugin: {}", plugin_name);

        if plugin.is_unique() && self.main().plugin_names.contains(plugin_name.as_str()) {
            log::debug!("duplicated plugin: {}", plugin_name);
            return Err(plugin);
        }

        let index = self.main().plugins.len();
        self.main_mut().plugins.push(Box::new(PlaceholderPlugin));
        self.main_mut().plugin_build_depth += 1;

        let f = AssertUnwindSafe(|| plugin.build(self));

        #[cfg(feature = "std")]
        let result = ::std::panic::catch_unwind(f);

        #[cfg(not(feature = "std"))]
        f();

        self.main_mut().plugin_build_depth -= 1;
        self.main_mut().plugin_names.insert(plugin_name);

        #[cfg(feature = "std")]
        if let Err(payload) = result {
            ::std::panic::resume_unwind(payload);
        }

        self.main_mut().plugins[index] = plugin;

        Ok(())
    }

    /// Returns `true` if a plugin of type `T` has been added.
    pub fn is_plugin_added<T>(&self) -> bool
    where
        T: Plugin,
    {
        self.main().is_plugin_added::<T>()
    }

    /// Returns all added plugin instances that match type `T`.
    pub fn get_added_plugins<T>(&self) -> Vec<&T>
    where
        T: Plugin,
    {
        self.main().get_added_plugins::<T>()
    }

    /// Extract data from the main world into the [`SubApp`] with the given label and perform an update if it exists.
    pub fn update_sub_app_by_label(&mut self, label: impl AppLabel) {
        self.sub_apps.update_subapp_by_label(label);
    }

    /// Registers a component type in the main world.
    pub fn register_components<T: Component>(&mut self) -> &mut Self {
        self.world_mut().register_component::<T>();
        self
    }

    /// Registers reflected type metadata for `T` in the app type registry.
    pub fn register_type<T: GetTypeMeta>(&mut self) -> &mut Self {
        self.main_mut().register_type::<T>();
        self
    }

    /// Registers type data `D` for reflected type `T` in the app type registry.
    pub fn register_type_data<T: voker_reflect::info::Typed, D: TypeData + FromType<T>>(
        &mut self,
    ) -> &mut Self {
        self.main_mut().register_type_data::<T, D>();
        self
    }

    /// Registers a fallible conversion route from `T` to `U` in the app type registry.
    pub fn register_type_conversion<T, U, F>(&mut self, function: F) -> &mut Self
    where
        T: Reflect + TypePath,
        U: Reflect + TypePath,
        F: Fn(T) -> Result<U, T> + Clone + Send + Sync + 'static,
    {
        self.main_mut().register_type_conversion::<T, U, F>(function);
        self
    }

    /// Registers an infallible `Into` conversion route from `T` to `U` in the app type registry.
    pub fn register_into_type_conversion<T, U>(&mut self) -> &mut Self
    where
        T: Reflect + TypePath,
        U: Reflect + TypePath + From<T>,
    {
        self.main_mut().register_into_type_conversion::<T, U>();
        self
    }

    /// Registers a resource type in the main world.
    pub fn register_resource<T: Resource>(&mut self) -> &mut Self {
        self.world_mut().register_resource::<T>();
        self
    }

    /// Ensures a schedule with `label` exists in the main world.
    pub fn init_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        self.main_mut().init_schedule(label);
        self
    }

    /// Edits the schedule identified by `label` in place.
    pub fn edit_schedule(
        &mut self,
        label: impl ScheduleLabel,
        f: impl FnMut(&mut Schedule),
    ) -> &mut Self {
        self.main_mut().edit_schedule(label, f);
        self
    }

    /// Inserts or replaces a schedule in the main world.
    pub fn insert_schedule(&mut self, schedule: Schedule) -> &mut Self {
        self.main_mut().insert_schedule(schedule);
        self
    }

    /// Returns an immutable schedule by label from the main world.
    pub fn get_schedule(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        self.main().get_schedule(label)
    }

    /// Returns a mutable schedule by label from the main world.
    pub fn get_schedule_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        self.main_mut().get_schedule_mut(label)
    }

    /// Adds a global observer to the main world.
    pub fn add_observer<M>(&mut self, observer: impl IntoObserver<M>) -> &mut Self {
        self.world_mut().add_observer(observer);
        self
    }

    /// Returns a pending exit status if one has been requested.
    pub fn should_exit(&self) -> Option<AppExit> {
        let messages = self.world().get_resource::<MessageQueue<AppExit>>()?;

        if !messages.is_empty() {
            let ret = MessageCursor::new(messages)
                .read(messages)
                .copied()
                .find(|exit| exit.is_error())
                .unwrap_or(AppExit::Success);
            return Some(ret);
        }

        None
    }

    /// Returns the currently configured app-level error handler.
    pub fn get_error_handler(&self) -> Option<ErrorHandler> {
        self.error_handler
    }

    /// Sets the fallback error handler on this app and all sub-app worlds.
    pub fn set_error_handler(&mut self, handler: ErrorHandler) -> &mut Self {
        self.error_handler = Some(handler);
        self.main_mut()
            .world_mut()
            .insert_resource(FallbackErrorHandler(handler));
        self.sub_apps.sub_apps.values_mut().for_each(|app| {
            app.world_mut().insert_resource(FallbackErrorHandler(handler));
        });
        self
    }
}

impl SubApps {
    /// Updates the main app and then all registered sub-apps.
    pub fn update(&mut self) {
        self.main.run_default_schedule();

        for (_, sub_app) in self.sub_apps.iter_mut() {
            sub_app.extract(&mut self.main.world);
            sub_app.update();
        }

        self.main.world.clear_trackers();
    }

    /// Iterates over the main app and all sub-apps.
    pub fn iter(&self) -> impl Iterator<Item = &SubApp> + '_ {
        core::iter::once(&self.main).chain(self.sub_apps.values())
    }

    /// Iterates mutably over the main app and all sub-apps.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SubApp> + '_ {
        core::iter::once(&mut self.main).chain(self.sub_apps.values_mut())
    }

    /// Updates one sub-app by label after running extraction from the main world.
    pub fn update_subapp_by_label(&mut self, label: impl AppLabel) {
        if let Some(sub) = self.sub_apps.get_mut(&label.intern()) {
            sub.extract(self.main.world_mut());
            sub.update();
        }
    }
}

fn run_once(mut app: App) -> AppExit {
    if app.plugins_state() != PluginsState::Cleaned {
        while app.plugins_state() == PluginsState::Adding {
            let ticker = voker_task::TaskPool::local_ticker();
            while ticker.try_tick() {}
        }
        app.finish();
        app.cleanup();
    }

    app.update();

    app.should_exit().unwrap_or(AppExit::Success)
}
