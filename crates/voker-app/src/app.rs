use alloc::boxed::Box;
use core::fmt::Debug;
use core::panic::AssertUnwindSafe;

use voker_ecs::component::Component;
use voker_ecs::error::{ErrorHandler, FallbackErrorHandler};
use voker_ecs::message::{Message, MessageCursor, MessageQueue};
use voker_ecs::observer::IntoObserver;
use voker_ecs::reflect::AppTypeRegistry;
use voker_ecs::resource::Resource;
use voker_ecs::schedule::{IntoSystemConfig, Schedule, ScheduleLabel};
use voker_ecs::system::IntoSystem;
use voker_ecs::world::{FromWorld, World};
use voker_reflect::registry::{FromType, GetTypeMeta, TypeData};
use voker_reflect::{Reflect, info::TypePath};
use voker_utils::hash::HashMap;

use crate::MainSchedulePlugin;
use crate::plugin::PlaceholderPlugin;
use crate::{AppExit, SubApp};
use crate::{AppLabel, InternedAppLabel};
use crate::{Plugin, Plugins, PluginsState};

type RunnerFn = Box<dyn FnOnce(App) -> AppExit>;

/// [`App`] is the primary API for writing user applications.
///
/// It automates the setup of a [standard lifecycle](crate::Main)
/// and provides interface glue for [plugins](Plugin).
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
/// ```no_run
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
    pub(crate) main: SubApp,
    pub(crate) sub_apps: HashMap<InternedAppLabel, SubApp>,
    pub(crate) runner: RunnerFn,
    pub(crate) error_handler: Option<ErrorHandler>,
}

impl Debug for App {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("App")
            .field("sub_apps", &self.sub_apps.keys())
            .finish()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Creates an empty app without world and plugins.
    pub fn empty() -> Self {
        Self {
            main: SubApp::empty(),
            sub_apps: HashMap::new(),
            runner: Box::new(run_once),
            error_handler: None,
        }
    }

    /// Creates a new app with the default setup.
    pub fn new() -> Self {
        let mut app = Self {
            main: SubApp::new(),
            sub_apps: HashMap::new(),
            runner: Box::new(run_once),
            error_handler: None,
        };

        app.add_plugins(MainSchedulePlugin);
        app.add_message::<AppExit>();
        app.world_mut()
            .resource_mut_or_init::<AppTypeRegistry>()
            .auto_register();

        app
    }

    /// Returns the main sub-app.
    pub fn main(&self) -> &SubApp {
        &self.main
    }

    /// Returns mutable access to the main sub-app.
    pub fn main_mut(&mut self) -> &mut SubApp {
        &mut self.main
    }

    /// Returns the world of the main sub-app.
    pub fn world(&self) -> &World {
        self.main.world()
    }

    /// Returns mutable access to the world of the main sub-app.
    pub fn world_mut(&mut self) -> &mut World {
        self.main.world_mut()
    }

    /// Returns all sub-app containers.
    pub fn sub_apps(&self) -> &HashMap<InternedAppLabel, SubApp> {
        &self.sub_apps
    }

    /// Returns mutable access to all sub-app containers.
    pub fn sub_apps_mut(&mut self) -> &mut HashMap<InternedAppLabel, SubApp> {
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
        self.sub_apps.get(&label.intern())
    }

    /// Returns mutable access to the sub-app for `label`, if it exists.
    pub fn get_sub_app_mut(&mut self, label: impl AppLabel) -> Option<&mut SubApp> {
        self.sub_apps.get_mut(&label.intern())
    }

    /// Inserts or replaces a sub-app at `label`.
    pub fn insert_sub_app(&mut self, label: impl AppLabel, mut sub_app: SubApp) -> &mut Self {
        if let Some(handler) = self.error_handler {
            let world = sub_app.world_mut();
            if !world.contains_resource::<FallbackErrorHandler>() {
                world.insert_resource(FallbackErrorHandler(handler));
            }
        }

        self.sub_apps.insert(label.intern(), sub_app);
        self
    }

    /// Removes and returns the sub-app at `label`, if present.
    pub fn remove_sub_app(&mut self, label: impl AppLabel) -> Option<SubApp> {
        self.sub_apps.remove(&label.intern())
    }

    /// Sets the function used when [`App::run`] is called.
    pub fn set_runner(&mut self, runner: impl FnOnce(App) -> AppExit + 'static) -> &mut Self {
        self.runner = Box::new(runner);
        self
    }

    /// Runs the app by consuming the configured runner.
    pub fn run(&mut self) -> AppExit {
        use core::mem::replace;

        if self.is_building_plugins() {
            core::hint::cold_path();
            panic!("App::run() was called while a plugin was building.");
        }

        let runner = replace(&mut self.runner, Box::new(run_once));
        let app = replace(self, App::empty());
        (runner)(app)
    }

    /// Runs one update step for this app and all registered sub-apps.
    ///
    /// Before execution, the plugin must have been built.
    pub fn update(&mut self) {
        debug_assert!(!self.is_building_plugins());

        self.main.run_main_schedule();

        let world = self.main.world_mut();
        for sub_app in self.sub_apps.values_mut() {
            sub_app.extract(world);
            sub_app.update();
        }

        world.clear_trackers();
    }

    /// Returns `true` if plugin build is in progress in main or any sub-app.
    pub fn is_building_plugins(&self) -> bool {
        self.main.is_building_plugins() || self.sub_apps.values().any(SubApp::is_building_plugins)
    }

    /// Returns the aggregate plugin state across main and sub-apps.
    pub fn plugins_state(&mut self) -> PluginsState {
        let mut overall = self.main_mut().plugins_state();
        self.sub_apps
            .values_mut()
            .for_each(|app| overall = overall.min(app.plugins_state()));
        overall
    }

    /// Advances plugin lifecycle until all plugins are built, finished, and cleaned up.
    ///
    /// This repeatedly polls local task execution while waiting for asynchronous
    /// plugin readiness transitions.
    pub fn build_plugins(&mut self) {
        if self.plugins_state() != PluginsState::Cleaned {
            while self.plugins_state() == PluginsState::Adding {
                let ticker = voker_task::TaskPool::local_ticker();
                while ticker.try_tick() {}
                if self.plugins_state() == PluginsState::Adding {
                    panic!("The plugin building cannot be completed.")
                }
            }
            self.finish();
            self.cleanup();
        }
    }

    /// Calls [`Plugin::finish`] on all loaded plugins.
    pub fn finish(&mut self) {
        let mut placeholder: Box<dyn Plugin> = Box::new(PlaceholderPlugin);
        for i in 0..self.main.plugins.len() {
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
            placeholder.finish(self);
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
        }
        self.main_mut().plugins_state = PluginsState::Finished;

        self.sub_apps.values_mut().for_each(SubApp::finish);
    }

    /// Calls [`Plugin::cleanup`] on all loaded plugins.
    pub fn cleanup(&mut self) {
        let mut placeholder: Box<dyn Plugin> = Box::new(PlaceholderPlugin);
        for i in 0..self.main.plugins.len() {
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
            placeholder.cleanup(self);
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
        }
        self.main_mut().plugins_state = PluginsState::Cleaned;

        self.sub_apps.values_mut().for_each(SubApp::cleanup);
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

    /// Registers a component type in the main world.
    pub fn register_component<T: Component>(&mut self) -> &mut Self {
        self.world_mut().register_component::<T>();
        self
    }

    /// Ensures a schedule with `label` exists in the main world.
    pub fn init_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        self.main_mut().init_schedule(label.intern());
        self
    }

    /// Inserts or replaces a schedule in the main world.
    pub fn insert_schedule(&mut self, schedule: Schedule) -> &mut Self {
        self.main_mut().insert_schedule(schedule);
        self
    }

    /// Returns a mutable reference to the schedule associated with label, if it exists.
    pub fn get_schedule_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        self.main_mut().get_schedule_mut(label.intern())
    }

    /// Returns a reference to the schedule associated with label, if it exists.
    pub fn get_schedule(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        self.main().get_schedule(label.intern())
    }

    /// Edits the schedule identified by `label` in place.
    pub fn edit_schedule(
        &mut self,
        label: impl ScheduleLabel,
        f: impl FnMut(&mut Schedule),
    ) -> &mut Self {
        self.main_mut().edit_schedule(label.intern(), f);
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

    /// Registers a message type in the main world.
    pub fn add_message<M: Message>(&mut self) -> &mut Self {
        self.main_mut().add_message::<M>();
        self
    }

    /// Adds a global observer to the main world.
    pub fn add_observer<M>(&mut self, observer: impl IntoObserver<M>) -> &mut Self {
        self.world_mut().add_observer(observer);
        self
    }

    /// Adds plugins or plugin groups to this app.
    pub fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        if matches!(
            self.plugins_state(),
            PluginsState::Finished | PluginsState::Cleaned
        ) {
            core::hint::cold_path();
            panic!("Plugins cannot be added after App::finish() or App::cleanup().");
        }
        plugins.add_to_app(self);
        self
    }

    pub(crate) fn add_boxed_plugin(
        &mut self,
        plugin: Box<dyn Plugin>,
    ) -> Result<(), Box<dyn Plugin>> {
        let plugin_name = plugin.name();
        log::debug!("added plugin: {}", plugin_name);

        if self.main.plugin_names.contains(plugin_name) {
            log::debug!("duplicated plugin: {}", plugin_name);
            return Err(plugin);
        }

        let index = self.main().plugins.len();
        self.main_mut().plugins.push(Box::new(PlaceholderPlugin));
        self.main_mut().plugin_names.insert(plugin_name);
        self.main_mut().plugin_build_depth += 1;

        let f = AssertUnwindSafe(|| plugin.build(self));

        #[cfg(feature = "std")]
        let result = ::std::panic::catch_unwind(f);

        #[cfg(not(feature = "std"))]
        f();

        self.main_mut().plugin_build_depth -= 1;

        #[cfg(feature = "std")]
        if let Err(payload) = result {
            ::std::panic::resume_unwind(payload);
        }

        self.main_mut().plugins[index] = plugin;

        Ok(())
    }

    /// Returns `true` if a plugin of type `T` has been added.
    pub fn is_plugin_added<T: Plugin>(&self) -> bool {
        self.main().is_plugin_added::<T>()
    }

    /// Returns all added plugin instances that match type `T`.
    pub fn get_added_plugin<T: Plugin>(&self) -> Option<&T> {
        self.main().get_added_plugin::<T>()
    }

    /// Extract data from the main world into the [`SubApp`] with the given label and perform an update if it exists.
    pub fn update_sub_app(&mut self, label: impl AppLabel) {
        if let Some(sub_app) = self.sub_apps.get_mut(&label.intern()) {
            let world = self.main.world_mut();
            sub_app.extract(world);
            sub_app.update();
        }
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
    pub fn error_handler(&self) -> Option<ErrorHandler> {
        self.error_handler
    }

    /// Sets the fallback error handler on this app and all sub-app worlds.
    pub fn set_error_handler(&mut self, handler: ErrorHandler) -> &mut Self {
        self.error_handler = Some(handler);
        self.world_mut().insert_resource(FallbackErrorHandler(handler));
        self.sub_apps.values_mut().for_each(|app| {
            app.world_mut().insert_resource(FallbackErrorHandler(handler));
        });
        self
    }
}

fn run_once(mut app: App) -> AppExit {
    app.build_plugins();

    if app.is_building_plugins() {
        core::hint::cold_path();
        panic!("App::run_once() was called while a plugin was building.");
    }

    app.update();
    app.should_exit().unwrap_or(AppExit::Success)
}
