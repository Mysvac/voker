use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Debug;
use voker_ecs::world::World;

use voker_ecs::error::ErrorHandler;
use voker_ecs::error::FallbackErrorHandler;
use voker_ecs::resource::Resource;
use voker_ecs::schedule::Schedule;
use voker_ecs::schedule::{IntoSystemConfig, ScheduleLabel};
use voker_ecs::system::{IntoSystem, SystemError, SystemInput};
use voker_utils::hash::HashMap;

use crate::AppExit;
use crate::InternedAppLabel;
use crate::MainSchedulePlugin;
use crate::Plugin;
use crate::Plugins;
use crate::PluginsState;
use crate::SubApp;
use crate::main_schedule::Main;

type RunnerFn = Box<dyn FnOnce(App) -> AppExit>;

#[must_use]
pub struct App {
    pub sub_apps: SubApps,
    pub runner: RunnerFn,
    pub error_handler: Option<ErrorHandler>,
}

pub struct SubApps {
    /// The primary sub-app that contains the "main" world.
    pub main: SubApp,
    pub sub_apps: HashMap<InternedAppLabel, SubApp>,
}

impl Debug for App {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("App")
            .field("sub_apps", &self.sub_apps.sub_apps.len())
            .finish()
    }
}

impl Default for App {
    fn default() -> Self {
        let mut app = App::empty();
        app.sub_apps.main.update_schedule = Some(Main.intern());
        app.add_plugins(MainSchedulePlugin);
        app
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

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

    pub fn main(&self) -> &SubApp {
        &self.sub_apps.main
    }

    pub fn main_mut(&mut self) -> &mut SubApp {
        &mut self.sub_apps.main
    }

    pub fn world(&self) -> &World {
        self.main().world()
    }

    pub fn world_mut(&mut self) -> &mut World {
        self.main_mut().world_mut()
    }

    pub fn set_runner(&mut self, runner: impl FnOnce(App) -> AppExit + 'static) -> &mut Self {
        self.runner = Box::new(runner);
        self
    }

    pub fn update(&mut self) {
        if self.is_building_plugins() {
            voker_utils::cold_path();
            panic!("App::update() was called while a plugin was building.");
        }
        self.sub_apps.update();
    }

    pub fn run(&mut self) -> AppExit {
        if self.is_building_plugins() {
            voker_utils::cold_path();
            panic!("App::run() was called while a plugin was building.");
        }

        let runner = core::mem::replace(&mut self.runner, Box::new(run_once));
        let app = core::mem::replace(self, App::empty());
        (runner)(app)
    }

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

    pub fn insert_resource<R: Resource + Send>(&mut self, resource: R) -> &mut Self {
        self.main_mut().insert_resource(resource);
        self
    }

    pub fn init_resource<R: Resource + Send + voker_ecs::world::FromWorld>(&mut self) -> &mut Self {
        self.main_mut().init_resource::<R>();
        self
    }

    pub fn insert_non_send<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.main_mut().insert_non_send(resource);
        self
    }

    pub fn init_non_send<R: Resource + voker_ecs::world::FromWorld>(&mut self) -> &mut Self {
        self.main_mut().init_non_send::<R>();
        self
    }

    pub fn init_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        self.main_mut().init_schedule(label);
        self
    }

    pub fn edit_schedule(
        &mut self,
        label: impl ScheduleLabel,
        f: impl FnMut(&mut Schedule),
    ) -> &mut Self {
        self.main_mut().edit_schedule(label, f);
        self
    }

    pub fn insert_schedule(&mut self, schedule: Schedule) -> &mut Self {
        self.main_mut().insert_schedule(schedule);
        self
    }

    pub fn remove_schedule(&mut self, label: impl ScheduleLabel) -> Option<Schedule> {
        self.main_mut().remove_schedule(label)
    }

    pub fn get_schedule(&self, label: impl ScheduleLabel) -> Option<&Schedule> {
        self.main().get_schedule(label)
    }

    pub fn get_schedule_mut(&mut self, label: impl ScheduleLabel) -> Option<&mut Schedule> {
        self.main_mut().get_schedule_mut(label)
    }

    pub fn run_schedule(&mut self, label: impl ScheduleLabel) -> &mut Self {
        self.main_mut().run_schedule(label);
        self
    }

    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        systems: impl IntoSystemConfig<M>,
    ) -> &mut Self {
        self.main_mut().add_systems(label, systems);
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
        self.main_mut().register_system(system);
        self
    }

    pub fn run_system<O: 'static, M>(
        &mut self,
        system: impl IntoSystem<(), O, M> + 'static,
    ) -> Result<O, SystemError> {
        self.main_mut().run_system(system)
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
        self.main_mut().run_system_with(system, input)
    }

    pub(crate) fn add_boxed_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        let plugin_name = plugin.name().to_string();

        if plugin.is_unique() && self.main().plugin_names.contains(plugin_name.as_str()) {
            return Err(plugin_name);
        }

        self.main_mut().plugin_build_depth += 1;
        plugin.build(self);
        self.main_mut().plugin_build_depth -= 1;

        let main = self.main_mut();
        main.plugin_names.insert(plugin_name);
        main.plugins.push(plugin);

        Ok(())
    }

    pub fn is_plugin_added<T>(&self) -> bool
    where
        T: Plugin,
    {
        self.main().plugin_names.contains(core::any::type_name::<T>())
    }

    pub fn get_added_plugins<T>(&self) -> Vec<&T>
    where
        T: Plugin,
    {
        self.main()
            .plugins
            .iter()
            .filter_map(|p| (p.as_ref() as &dyn core::any::Any).downcast_ref::<T>())
            .collect()
    }

    pub fn plugins_state(&mut self) -> PluginsState {
        let mut overall = self.main_mut().plugins_state();
        self.sub_apps
            .sub_apps
            .values_mut()
            .for_each(|app| overall = overall.min(app.plugins_state()));
        overall
    }

    pub fn finish(&mut self) {
        struct Placeholder;
        impl Plugin for Placeholder {
            fn build(&self, _app: &mut App) {}
        }

        let mut placeholder: Box<dyn Plugin> = Box::new(Placeholder);
        for i in 0..self.main().plugins.len() {
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
            placeholder.finish(self);
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
        }
        self.main_mut().plugins_state = PluginsState::Finished;

        self.sub_apps.sub_apps.values_mut().for_each(SubApp::finish);
    }

    pub fn cleanup(&mut self) {
        struct Placeholder;
        impl Plugin for Placeholder {
            fn build(&self, _app: &mut App) {}
        }

        let mut placeholder: Box<dyn Plugin> = Box::new(Placeholder);
        for i in 0..self.main().plugins.len() {
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
            placeholder.cleanup(self);
            core::mem::swap(&mut self.main_mut().plugins[i], &mut placeholder);
        }
        self.main_mut().plugins_state = PluginsState::Cleaned;

        self.sub_apps.sub_apps.values_mut().for_each(SubApp::cleanup);
    }

    pub fn insert_sub_app(&mut self, label: impl crate::AppLabel, mut app: SubApp) -> &mut Self {
        if let Some(handler) = self.error_handler {
            app.world_mut().insert_resource(FallbackErrorHandler(handler));
        }

        let _old = self.sub_apps.sub_apps.insert(label.intern(), app);
        self
    }

    pub fn remove_sub_app(&mut self, label: impl crate::AppLabel) -> Option<SubApp> {
        self.sub_apps.sub_apps.remove(&label.intern())
    }

    pub fn get_sub_app(&self, label: impl crate::AppLabel) -> Option<&SubApp> {
        self.sub_apps.sub_apps.get(&label.intern())
    }

    pub fn get_sub_app_mut(&mut self, label: impl crate::AppLabel) -> Option<&mut SubApp> {
        self.sub_apps.sub_apps.get_mut(&label.intern())
    }

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

    pub(crate) fn is_building_plugins(&self) -> bool {
        self.sub_apps.main.is_building_plugins()
            || self.sub_apps.sub_apps.values().any(SubApp::is_building_plugins)
    }
}

impl SubApps {
    pub fn update(&mut self) {
        self.main.update();

        let labels: Vec<_> = self.sub_apps.keys().copied().collect();
        for label in labels {
            if let Some(sub) = self.sub_apps.get_mut(&label) {
                sub.extract(self.main.world_mut());
                sub.update();
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &SubApp> + '_ {
        core::iter::once(&self.main).chain(self.sub_apps.values())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut SubApp> + '_ {
        core::iter::once(&mut self.main).chain(self.sub_apps.values_mut())
    }

    pub fn update_subapp_by_label(&mut self, label: impl crate::AppLabel) {
        if let Some(sub) = self.sub_apps.get_mut(&label.intern()) {
            sub.extract(self.main.world_mut());
            sub.update();
        }
    }
}

fn run_once(mut app: App) -> AppExit {
    app.update();
    AppExit::Success
}
