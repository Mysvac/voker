use crate::{App, Plugin};

/// Adds sensible panic handlers to apps. Adding
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
/// #     fn builder(self) -> PluginGroupBuilder {
/// #         PluginGroupBuilder::start::<Self>().add(PanicHandlerPlugin)
/// #     }
/// # }
/// fn main() {
///     App::new()
///         .add_plugins(BasePlugins.builder().disable::<PanicHandlerPlugin>())
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
