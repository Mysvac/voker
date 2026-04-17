//! This module provides panic handlers for apps, and automatically configures platform specifics (i.e. Wasm or Android).
//!
//! You can add [`PanicHandlerPlugin`] directly, or include it via your own
//! [`PluginGroup`](crate::PluginGroup).

use crate::{App, Plugin};

/// Adds sensible panic handlers to apps. Adding
/// this plugin will setup a panic hook appropriate to your target platform:
/// * On Wasm, uses [`console_error_panic_hook`](https://crates.io/crates/console_error_panic_hook), logging
///   to the browser console.
/// * Other platforms are currently not setup.
///
/// ```no_run
/// # use voker_app::{App, NoopPluginGroup, PanicHandlerPlugin};
/// fn main() {
///     App::new()
///         .add_plugins(NoopPluginGroup)
///         .add_plugins(PanicHandlerPlugin)
///         .run();
/// }
/// ```
///
/// If you want to setup your own panic handler, you should disable this
/// plugin from your plugin group:
/// ```no_run
/// # use voker_app::{App, PanicHandlerPlugin, PluginGroup, PluginGroupBuilder};
/// #
/// # struct BasePlugins;
/// # impl PluginGroup for BasePlugins {
/// #     fn build(self) -> PluginGroupBuilder {
/// #         PluginGroupBuilder::start::<Self>().add(PanicHandlerPlugin)
/// #     }
/// # }
/// fn main() {
///     App::new()
///         .add_plugins(BasePlugins.build().disable::<PanicHandlerPlugin>())
///         .run();
/// }
/// ```
#[derive(Default)]
pub struct PanicHandlerPlugin;

impl Plugin for PanicHandlerPlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(feature = "std")]
        {
            static SET_HOOK: std::sync::Once = std::sync::Once::new();

            SET_HOOK.call_once(|| {
                voker_cfg::switch! {
                    crate::cfg::web => {
                        std::panic::set_hook(alloc::boxed::Box::new(console_error_panic_hook::hook));
                    }
                    voker_ecs::cfg::backtrace => {
                        let current_hook = std::panic::take_hook();
                        std::panic::set_hook(alloc::boxed::Box::new(
                            voker_ecs::error::game_error_panic_hook(current_hook),
                        ));
                    }
                    _ => {
                        // Otherwise use the default target panic hook - Do nothing.
                    }
                }
            });
        }
    }
}
