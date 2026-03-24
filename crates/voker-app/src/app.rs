use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use voker_ecs::{error::ErrorHandler, schedule::InternedScheduleLabel, world::World};
use voker_utils::hash::{HashMap, HashSet};

use crate::{InternedAppLabel, Plugin, PluginsState};

#[must_use]
pub struct App {
    pub sub_apps: SubApps,
    pub runner: AppRunner,
    pub default_error_handler: Option<ErrorHandler>,
}

pub struct SubApps {
    /// The primary sub-app that contains the "main" world.
    pub main: SubApp,
    pub sub_apps: HashMap<InternedAppLabel, SubApp>,
}

pub struct AppRunner {
    pub main: SubApp,
    pub sub_apps: HashMap<InternedAppLabel, SubApp>,
}

pub struct SubApp {
    pub world: World,
    pub plugin_registry: Vec<Box<dyn Plugin>>,
    pub plugin_names: HashSet<String>,
    pub plugin_build_depth: usize,
    pub plugins_state: PluginsState,
    pub update_schedule: Option<InternedScheduleLabel>,
    pub extract: Option<Box<dyn FnMut(&mut World, &mut World) + Send>>,
}
