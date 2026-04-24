use crate::Plugin;
use crate::{App, AppExit};

use core::time::Duration;
use voker_os::time::Instant;

// -----------------------------------------------------------------------------
// RunMode

/// Determines the method used to run an [`App`]'s [`Schedule`].
///
/// It is used in the [`ScheduleRunnerPlugin`], default is loop without waiting.
///
/// [`Schedule`]: voker_ecs::schedule::Schedule
#[derive(Copy, Clone, Debug)]
pub enum RunMode {
    /// Indicates that the [`App`]'s schedule should run repeatedly.
    Loop {
        /// The minimum [`Duration`] to wait after a [`Schedule`]
        /// has completed before repeating.
        ///
        /// A value of [`None`] will not wait.
        ///
        /// [`Schedule`]: voker_ecs::schedule::Schedule
        wait: Option<Duration>,
    },
    /// Indicates that the [`App`]'s schedule should run only once.
    Once,
}

impl Default for RunMode {
    fn default() -> Self {
        RunMode::Loop { wait: None }
    }
}

// -----------------------------------------------------------------------------
// ScheduleRunnerPlugin

/// Configures an [`App`] to run its [`Schedule`] according to a given [`RunMode`].
///
/// Add this plugin when your app should actively drive the update loop itself.
/// In environments where another runtime already owns the loop, this plugin may
/// be unnecessary.
///
/// [`Schedule`]: voker_ecs::schedule::Schedule
#[derive(Default)]
pub struct ScheduleRunnerPlugin {
    /// Determines whether the [`Schedule`](voker_ecs::schedule::Schedule) is run once or repeatedly.
    pub run_mode: RunMode,
}

impl ScheduleRunnerPlugin {
    /// See [`RunMode::Once`].
    pub const fn run_once() -> Self {
        ScheduleRunnerPlugin {
            run_mode: RunMode::Once,
        }
    }

    /// See [`RunMode::Loop`].
    pub const fn run_loop(wait_duration: Option<Duration>) -> Self {
        ScheduleRunnerPlugin {
            run_mode: RunMode::Loop {
                wait: wait_duration,
            },
        }
    }
}

impl Plugin for ScheduleRunnerPlugin {
    fn build(&self, app: &mut App) {
        let run_mode = self.run_mode;

        app.set_runner(move |mut app: App| {
            app.build_plugins();

            if app.is_building_plugins() {
                core::hint::cold_path();
                panic!(
                    "ScheduleRunnerPlugin: `App::run()` was called while a plugin was building."
                );
            }

            match run_mode {
                RunMode::Once => {
                    app.update();
                    app.should_exit().unwrap_or(AppExit::Success)
                }
                RunMode::Loop { wait: None } => {
                    // We specialize for loops that do not require waiting.
                    // Then the loop does not need to obtain the time stamp.
                    loop {
                        app.update();
                        if let Some(exit) = app.should_exit() {
                            return exit;
                        }
                    }
                }
                RunMode::Loop { wait: Some(wait) } => {
                    // Separate to reduce the size of the runner itself.
                    fn tick(app: &mut App, wait: Duration) -> Result<Option<Duration>, AppExit> {
                        let start_time = Instant::now();

                        app.update();

                        if let Some(exit) = app.should_exit() {
                            return Err(exit);
                        };

                        let exe_time = start_time.elapsed();
                        if exe_time < wait {
                            return Ok(Some(wait - exe_time));
                        }

                        Ok(None)
                    }

                    loop {
                        match tick(&mut app, wait) {
                            Ok(Some(delay)) => {
                                voker_os::thread::sleep(delay);
                            }
                            Ok(None) => continue,
                            Err(exit) => return exit,
                        }
                    }
                }
            }
        });
    }
}
