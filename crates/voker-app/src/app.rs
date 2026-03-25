use alloc::boxed::Box;

use voker_ecs::error::ErrorHandler;
use voker_utils::hash::HashMap;

use crate::AppExit;
use crate::InternedAppLabel;
use crate::SubApp;

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

impl App {
    pub fn empty() -> Self {
        todo!()
    }
}
