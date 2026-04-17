use crate::Plugin;
use crate::PluginsState;
use crate::{App, AppExit};

use core::time::Duration;
use voker_os::time::Instant;

#[cfg(all(target_arch = "wasm32", feature = "web"))]
use {
    alloc::{boxed::Box, rc::Rc},
    core::cell::RefCell,
    wasm_bindgen::{JsCast, prelude::*},
};

/// Determines the method used to run an [`App`]'s [`Schedule`](voker_ecs::schedule::Schedule).
///
/// It is used in the [`ScheduleRunnerPlugin`].
#[derive(Copy, Clone, Debug)]
pub enum RunMode {
    /// Indicates that the [`App`]'s schedule should run repeatedly.
    Loop {
        /// The minimum [`Duration`] to wait after a [`Schedule`](voker_ecs::schedule::Schedule)
        /// has completed before repeating. A value of [`None`] will not wait.
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

/// Configures an [`App`] to run its [`Schedule`](voker_ecs::schedule::Schedule) according to a given
/// [`RunMode`].
///
/// Add this plugin when your app should actively drive the update loop itself.
/// In environments where another runtime already owns the loop, this plugin may
/// be unnecessary.
#[derive(Default)]
pub struct ScheduleRunnerPlugin {
    /// Determines whether the [`Schedule`](voker_ecs::schedule::Schedule) is run once or repeatedly.
    pub run_mode: RunMode,
}

impl ScheduleRunnerPlugin {
    /// See [`RunMode::Once`].
    pub fn run_once() -> Self {
        ScheduleRunnerPlugin {
            run_mode: RunMode::Once,
        }
    }

    /// See [`RunMode::Loop`].
    pub fn run_loop(wait_duration: Duration) -> Self {
        ScheduleRunnerPlugin {
            run_mode: RunMode::Loop {
                wait: Some(wait_duration),
            },
        }
    }
}

impl Plugin for ScheduleRunnerPlugin {
    fn build(&self, app: &mut App) {
        let run_mode = self.run_mode;

        app.set_runner(move |mut app: App| {
            let plugins_state = app.plugins_state();

            if plugins_state != PluginsState::Cleaned {
                while app.plugins_state() == PluginsState::Adding {
                    crate::cfg::web! {
                        if {
                            // No need
                        } else {
                            let ticker = voker_task::TaskPool::local_ticker();
                            while ticker.try_tick() {}
                        }
                    }
                }
                app.finish();
                app.cleanup();
            }

            match run_mode {
                RunMode::Once => {
                    app.update();

                    if let Some(exit) = app.should_exit() {
                        return exit;
                    }

                    AppExit::Success
                }
                RunMode::Loop { wait } => {
                    let tick = move |app: &mut App,
                                     opt_wait: Option<Duration>|
                          -> Result<Option<Duration>, AppExit> {
                        let start_time = Instant::now();

                        app.update();

                        if let Some(exit) = app.should_exit() {
                            return Err(exit);
                        };

                        let end_time = Instant::now();

                        if let Some(wait) = opt_wait {
                            let exe_time = end_time - start_time;
                            if exe_time < wait {
                                return Ok(Some(wait - exe_time));
                            }
                        }

                        Ok(None)
                    };

                    crate::cfg::web! {
                        if {
                            fn set_timeout(callback: &Closure<dyn FnMut()>, dur: Duration) {
                                web_sys::window()
                                    .unwrap()
                                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                                        callback.as_ref().unchecked_ref(),
                                        dur.as_millis() as i32,
                                    )
                                    .expect("Should register `setTimeout`.");
                            }
                            let asap = Duration::from_millis(1);

                            let exit = Rc::new(RefCell::new(AppExit::Success));
                            let closure_exit = exit.clone();

                            let mut app = Rc::new(app);
                            let moved_tick_closure = Rc::new(RefCell::new(None));
                            let base_tick_closure = moved_tick_closure.clone();

                            let tick_app = move || {
                                let app = Rc::get_mut(&mut app).unwrap();
                                let delay = tick(app, wait);
                                match delay {
                                    Ok(delay) => set_timeout(
                                        moved_tick_closure.borrow().as_ref().unwrap(),
                                        delay.unwrap_or(asap),
                                    ),
                                    Err(code) => {
                                        closure_exit.replace(code);
                                    }
                                }
                            };
                            *base_tick_closure.borrow_mut() =
                                Some(Closure::wrap(Box::new(tick_app) as Box<dyn FnMut()>));
                            set_timeout(base_tick_closure.borrow().as_ref().unwrap(), asap);

                            exit.take()
                        } else {
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
                }
            }
        });
    }
}
