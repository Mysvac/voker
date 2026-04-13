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

impl<T: Fn(&mut App) + Send + Sync + 'static> Plugin for T {
    fn build(&self, app: &mut App) {
        self(app);
    }
}

pub trait Plugins<Marker>: sealed::Plugins<Marker> {}

impl<Marker, T> Plugins<Marker> for T where T: sealed::Plugins<Marker> {}

mod sealed {
    use alloc::boxed::Box;

    use crate::App;
    use crate::plugin::Plugin;

    pub trait Plugins<Marker> {
        fn add_to_app(self, app: &mut App);
    }

    pub struct PluginMarker;
    pub struct PluginsTupleMarker;

    impl<P: Plugin> Plugins<PluginMarker> for P {
        #[track_caller]
        fn add_to_app(self, app: &mut App) {
            if let Err(plugin_name) = app.add_boxed_plugin(Box::new(self)) {
                panic!(
                    "Error adding plugin {plugin_name}: plugin was already added in application"
                );
            }
        }
    }

    impl Plugins<(PluginsTupleMarker,)> for () {
        fn add_to_app(self, _app: &mut App) {}
    }

    impl<A, MA> Plugins<(PluginsTupleMarker, MA)> for (A,)
    where
        A: super::Plugins<MA>,
    {
        fn add_to_app(self, app: &mut App) {
            let (a,) = self;
            a.add_to_app(app);
        }
    }

    impl<A, B, MA, MB> Plugins<(PluginsTupleMarker, MA, MB)> for (A, B)
    where
        A: super::Plugins<MA>,
        B: super::Plugins<MB>,
    {
        fn add_to_app(self, app: &mut App) {
            let (a, b) = self;
            a.add_to_app(app);
            b.add_to_app(app);
        }
    }

    impl<A, B, C, MA, MB, MC> Plugins<(PluginsTupleMarker, MA, MB, MC)> for (A, B, C)
    where
        A: super::Plugins<MA>,
        B: super::Plugins<MB>,
        C: super::Plugins<MC>,
    {
        fn add_to_app(self, app: &mut App) {
            let (a, b, c) = self;
            a.add_to_app(app);
            b.add_to_app(app);
            c.add_to_app(app);
        }
    }

    impl<A, B, C, D, MA, MB, MC, MD> Plugins<(PluginsTupleMarker, MA, MB, MC, MD)> for (A, B, C, D)
    where
        A: super::Plugins<MA>,
        B: super::Plugins<MB>,
        C: super::Plugins<MC>,
        D: super::Plugins<MD>,
    {
        fn add_to_app(self, app: &mut App) {
            let (a, b, c, d) = self;
            a.add_to_app(app);
            b.add_to_app(app);
            c.add_to_app(app);
            d.add_to_app(app);
        }
    }

    impl<A, B, C, D, E, MA, MB, MC, MD, ME> Plugins<(PluginsTupleMarker, MA, MB, MC, MD, ME)>
        for (A, B, C, D, E)
    where
        A: super::Plugins<MA>,
        B: super::Plugins<MB>,
        C: super::Plugins<MC>,
        D: super::Plugins<MD>,
        E: super::Plugins<ME>,
    {
        fn add_to_app(self, app: &mut App) {
            let (a, b, c, d, e) = self;
            a.add_to_app(app);
            b.add_to_app(app);
            c.add_to_app(app);
            d.add_to_app(app);
            e.add_to_app(app);
        }
    }
}
