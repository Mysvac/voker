use core::any::Any;

use crate::App;

/// Plugins state in the application
#[derive(PartialEq, Eq, Debug, Clone, Copy, PartialOrd, Ord)]
pub enum PluginsState {
    /// Plugins are being added.
    Adding,
    /// All plugins already added are ready.
    Ready,
    /// Finish has been executed for all plugins added.
    Finished,
    /// Cleanup has been executed for all plugins added.
    Cleaned,
}

pub trait Plugin: Any + Send + Sync {
    /// Configures the [`App`] to which this plugin is added.
    fn build(&self, app: &mut App);

    /// Has the plugin finished its setup? This can be useful for plugins that need something
    /// asynchronous to happen before they can finish their setup, like the initialization of a renderer.
    /// Once the plugin is ready, [`finish`](Plugin::finish) should be called.
    fn ready(&self, _app: &App) -> bool {
        true
    }

    /// Finish adding this plugin to the [`App`], once all plugins registered are ready. This can
    /// be useful for plugins that depends on another plugin asynchronous setup, like the renderer.
    fn finish(&self, _app: &mut App) {
        // do nothing
    }

    /// Runs after all plugins are built and finished, but before the app schedule is executed.
    /// This can be useful if you have some resource that other plugins need during their build step,
    /// but after build you want to remove it and send it to another thread.
    fn cleanup(&self, _app: &mut App) {
        // do nothing
    }

    /// Configures a name for the [`Plugin`] which is primarily used for checking plugin
    /// uniqueness and debugging.
    fn name(&self) -> &str {
        core::any::type_name::<Self>()
    }

    /// If the plugin can be meaningfully instantiated several times in an [`App`],
    /// override this method to return `false`.
    fn is_unique(&self) -> bool {
        true
    }
}
