use voker_ecs::message::MessageWriter;
use voker_os::sync::atomic::{AtomicU8, Ordering};

use crate::{App, AppExit, Plugin, Update};

pub use ctrlc;

// -----------------------------------------------------------------------------
// TerminalCtrlCHandlerPlugin

/// Indicates that all [`App`]'s should exit.
static SHOULD_EXIT: AtomicU8 = AtomicU8::new(0);

/// Gracefully handles `Ctrl+C` by emitting an [`AppExit`] message.
///
/// ```no_run
/// # use voker_app::*;
///
/// fn main() {
///     App::new()
///         .add_plugins(TerminalCtrlCHandlerPlugin)
///         .run();
/// }
/// ```
///
/// If you want to setup your own `Ctrl+C` handler, you should call the
/// [`TerminalCtrlCHandlerPlugin::gracefully_exit`] function in your handler so
/// the app still exits gracefully.
///
/// ```no_run
/// # use voker_app::*;
///
/// fn main() {
///     // Your own `Ctrl+C` handler
///     ctrlc::set_handler(move || {
///         // Other clean up code ...
///
///         TerminalCtrlCHandlerPlugin::gracefully_exit();
///     });
///
///     App::new()
///         .add_plugins(TerminalCtrlCHandlerPlugin)
///         .run();
/// }
/// ```
#[derive(Default)]
pub struct TerminalCtrlCHandlerPlugin;

// -----------------------------------------------------------------------------
// Plugin Implementation

impl TerminalCtrlCHandlerPlugin {
    /// When called the first time, it sends the [`AppExit`] event to all apps using
    /// this plugin to make them gracefully exit.
    ///
    /// If called more than once, it exits immediately.
    pub fn gracefully_exit() {
        if SHOULD_EXIT.fetch_add(1, Ordering::SeqCst) > 0 {
            log::error!("Received more than one ctrl+c. Skipping graceful shutdown.");
            std::process::exit(Self::EXIT_CODE.into());
        };
    }

    /// Sends a [`AppExit`] event when the user presses `Ctrl+C` on the terminal.
    pub fn exit_on_flag(mut app_exit_writer: MessageWriter<AppExit>) {
        if SHOULD_EXIT.load(Ordering::Relaxed) > 0 {
            app_exit_writer.write(AppExit::from_code(Self::EXIT_CODE));
        }
    }

    const EXIT_CODE: u8 = 130;
}

impl Plugin for TerminalCtrlCHandlerPlugin {
    fn build(&self, app: &mut App) {
        let result = ctrlc::try_set_handler(move || {
            TerminalCtrlCHandlerPlugin::gracefully_exit();
        });

        match result {
            Ok(()) => {}
            Err(ctrlc::Error::MultipleHandlers) => {
                log::info!(
                    "Skipping installing `Ctrl+C` handler as one was already installed. \
                    Please call `TerminalCtrlCHandlerPlugin::gracefully_exit` in your own \
                    `Ctrl+C` handler if you still want graceful exit on `Ctrl+C`."
                );
            }
            Err(err) => log::warn!("Failed to set `Ctrl+C` handler: {err}"),
        }

        app.add_systems(Update, TerminalCtrlCHandlerPlugin::exit_on_flag);
    }
}
