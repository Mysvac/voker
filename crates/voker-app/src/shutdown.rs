use core::sync::atomic::Ordering;

use voker_ecs::message::MessageWriter;
use voker_os::atomic::AtomicU32;
use voker_os::utils::SpinLock;

use crate::{App, AppExit, Plugin, Update};

/// Graceful shutdown plugin for terminal signal handling (Ctrl+C).
///
/// # Behavior
///
/// - First `Ctrl+C` or `gracefully_exit()` call → emits `AppExit` for clean shutdown
/// - Second call → forces immediate exit via `std::process::exit` (or custom handler)
///
/// # Usage
///
/// ```no_run
/// # use voker_app::{App, ShutdownPlugin};
/// App::new().add_plugins(ShutdownPlugin).run();
/// ```
///
/// See [`on_signal`], [`on_force`] and [`gracefully_exit`] for more information.
///
/// [`on_signal`]: ShutdownPlugin::on_signal
/// [`on_force`]: ShutdownPlugin::on_force
/// [`gracefully_exit`]: ShutdownPlugin::gracefully_exit
///
/// # Platform Support
/// Signal handler only works on Unix/Windows with `std` feature enabled.
/// On unsupported platforms, `on_signal` returns `None` (no-op).
///
/// # Thread Safety
/// Process-global atomic flag, safe to call from any thread.
#[derive(Default)]
pub struct ShutdownPlugin;

static SHOULD_EXIT: AtomicU32 = AtomicU32::new(0);
static FORCE_EXIT: SpinLock<Option<fn(i32) -> !>> = SpinLock::new(None);

// -----------------------------------------------------------------------------
// Plugin Implementation

impl ShutdownPlugin {
    pub const EXIT_CODE: u8 = 130;

    /// Sets a custom signal handler for system stop signals (for example `Ctrl+C`) on supported platforms.
    ///
    /// Returns:
    /// - `Some(true)` if the platform supports signal handlers and the handler was installed successfully.
    /// - `Some(false)` if the platform supports signal handlers but installation failed (for example, system access failed).
    /// - `None` if the platform is unsupported (signal handling is not available in this build configuration).
    ///
    /// By default the registered handler will call [`ShutdownPlugin::gracefully_exit()`]. Prefer keeping
    /// user-provided signal closures minimal and delegating to `gracefully_exit` from the closure.
    ///
    /// Calling this function will overwrite the original handler. If you want to
    /// setup your own custom handler, remember to add `gracefully_exit` at the end.
    ///
    /// ```no_run
    /// # use voker_app::ShutdownPlugin;
    /// ShutdownPlugin::on_signal(move || {
    ///     // Other clean up code ...
    ///     ShutdownPlugin::gracefully_exit();
    /// });
    /// ```
    pub fn on_signal(_handler: impl FnMut() + 'static + Send) -> Option<bool> {
        #[cfg(all(any(all(unix, not(target_os = "horizon")), windows), feature = "std"))]
        return Some(ctrlc::set_handler(_handler).is_ok());
        #[cfg(not(all(any(all(unix, not(target_os = "horizon")), windows), feature = "std")))]
        return None;
    }

    /// Set the forced exit behavior when [`ShutdownPlugin::gracefully_exit`]
    /// is triggered multiple times.
    ///
    /// This function is platform-independent and is guaranteed to succeed.
    ///
    /// By default, the plugin uses [`std::process::exit`] to forcibly terminate
    /// the process, which requires the `std` feature.
    ///
    /// In `no_std` environments the default forced-exit function is unset and must
    /// be provided by the user. This does not mean the application cannot terminate:
    /// calling `gracefully_exit` sets a termination flag, and an `App` will stop on
    /// the next frame when it observes that flag. The `on_force` hook exists to provide
    /// a way to forcibly abort programs that are stuck in infinite loops.
    pub fn on_force(handler: fn(i32) -> !) {
        *FORCE_EXIT.lock() = Some(handler);
    }

    /// When called the first time, it sends the [`AppExit`] event to all apps using
    /// this plugin to make them gracefully exit.
    ///
    /// If called more than once, it exits immediately through given `on_force` function.
    ///
    /// On supported platforms, `gracefully_exit` will be called automatically when
    /// the process receives a termination signal.
    ///
    /// On unsupported platforms, users must call this function manually to stop the program.
    pub fn gracefully_exit() {
        if SHOULD_EXIT.fetch_add(1, Ordering::SeqCst) > 0 {
            tracing::error!("Received more than one ctrl+c. Skipping graceful shutdown.");
            if let Some(func) = *FORCE_EXIT.lock() {
                (func)(ShutdownPlugin::EXIT_CODE.into())
            } else {
                tracing::error!(
                    "The platform does not support `std::process::exit`, wait for the current loop to end."
                );
            }
        };
    }
}

impl Plugin for ShutdownPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(all(any(all(unix, not(target_os = "horizon")), windows), feature = "std"))]
        match ctrlc::try_set_handler(ShutdownPlugin::gracefully_exit) {
            Ok(()) => {
                tracing::debug!("ShutdownPlugin: Default on_signal handler install succeed");
            }
            Err(ctrlc::Error::MultipleHandlers) => {
                tracing::info!(
                    "Skipping installing default terminal signal handler as one was already \
                    installed. Please call `ShutdownPlugin::gracefully_exit` in your own \
                    handler if you still want graceful exit."
                );
            }
            Err(err) => tracing::warn!("Failed to set `Ctrl+C` handler: {err}"),
        }

        #[cfg(feature = "std")]
        if let Some(mut func) = FORCE_EXIT.try_lock()
            && func.is_none()
        {
            *func = Some(std::process::exit);
            tracing::debug!("ShutdownPlugin: Default on_force handler install succeed");
        } else {
            tracing::info!(
                "Skipping installing default force termination handler as one was already installed."
            );
        }

        fn exit_on_flag(mut app_exit_writer: MessageWriter<AppExit>) {
            if SHOULD_EXIT.load(Ordering::Relaxed) > 0 {
                app_exit_writer.write(AppExit::from_code(ShutdownPlugin::EXIT_CODE));
            }
        }

        app.add_systems(Update, (), exit_on_flag);
    }
}
