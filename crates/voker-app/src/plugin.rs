use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::{Any, TypeId};

use voker_utils::extra::TypeIdMap;

use crate::App;

// -----------------------------------------------------------------------------
// Placeholder

pub(crate) struct PlaceholderPlugin;

impl Plugin for PlaceholderPlugin {
    fn build(&self, _app: &mut App) {}
}

// -----------------------------------------------------------------------------
// PluginsState

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

// -----------------------------------------------------------------------------
// Plugin

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

// -----------------------------------------------------------------------------
// PluginGroup

/// Combines multiple plugins into one configurable unit.
///
/// A plugin group is typically used to provide a reusable default plugin set,
/// while still allowing users to enable/disable or reorder members.
pub trait PluginGroup: Sized {
    /// Configures the plugins that belong to this group.
    fn build(self) -> PluginGroupBuilder;

    /// Returns the group name used in diagnostics.
    fn name() -> &'static str {
        core::any::type_name::<Self>()
    }

    /// Sets an existing plugin value in this group.
    ///
    /// # Panics
    /// Panics if the plugin type is not present in the group.
    fn set<T: Plugin>(self, plugin: T) -> PluginGroupBuilder {
        self.build().set(plugin)
    }
}

struct PluginEntry {
    plugin: Box<dyn Plugin>,
    enabled: bool,
}

impl PluginGroup for PluginGroupBuilder {
    fn build(self) -> PluginGroupBuilder {
        self
    }
}

/// Builder used to compose and adjust a plugin group.
pub struct PluginGroupBuilder {
    group_name: &'static str,
    plugins: TypeIdMap<PluginEntry>,
    order: Vec<TypeId>,
}

impl PluginGroupBuilder {
    /// Starts a new builder for a plugin group type.
    pub fn start<PG: PluginGroup>() -> Self {
        Self {
            group_name: PG::name(),
            plugins: Default::default(),
            order: Default::default(),
        }
    }

    /// Returns `true` if this builder contains the given plugin type.
    pub fn contains<T: Plugin>(&self) -> bool {
        self.plugins.contains(TypeId::of::<T>())
    }

    /// Returns `true` if this builder contains the given plugin and it is enabled.
    pub fn enabled<T: Plugin>(&self) -> bool {
        self.plugins.get(TypeId::of::<T>()).is_some_and(|entry| entry.enabled)
    }

    /// Sets the value of an existing plugin in this group.
    ///
    /// # Panics
    /// Panics if the plugin type is not present.
    pub fn set<T: Plugin>(self, plugin: T) -> Self {
        self.try_set(plugin).unwrap_or_else(|_| {
            panic!(
                "{} does not exist in this PluginGroup",
                core::any::type_name::<T>(),
            )
        })
    }

    /// Tries to set the value of an existing plugin in this group.
    pub fn try_set<T: Plugin>(mut self, plugin: T) -> Result<Self, (Self, T)> {
        match self.plugins.get_mut(TypeId::of::<T>()) {
            Some(entry) => {
                entry.plugin = Box::new(plugin);
                Ok(self)
            }
            None => Err((self, plugin)),
        }
    }

    /// Adds a plugin to the end of this group.
    ///
    /// If the plugin already exists, it is moved to the new position.
    #[expect(
        clippy::should_implement_trait,
        reason = "This is an explicit builder operation, not `+` semantics."
    )]
    pub fn add<T: Plugin>(mut self, plugin: T) -> Self {
        let target_index = self.order.len();
        self.order.push(TypeId::of::<T>());
        self.upsert_plugin_state(plugin, target_index);
        self
    }

    /// Adds a plugin to the end of this group, failing if it already exists.
    pub fn try_add<T: Plugin>(self, plugin: T) -> Result<Self, (Self, T)> {
        if self.contains::<T>() {
            return Err((self, plugin));
        }

        Ok(self.add(plugin))
    }

    /// Adds all plugins from another group to the end of this group.
    ///
    /// Existing plugin types are replaced and moved to the end.
    pub fn add_group(mut self, group: impl PluginGroup) -> Self {
        let Self {
            mut plugins, order, ..
        } = group.build();

        for plugin_id in order {
            let entry = plugins.remove(plugin_id).expect("Plugin id missing in group order.");
            self.upsert_plugin_entry_state(plugin_id, entry, self.order.len());
            self.order.push(plugin_id);
        }

        self
    }

    /// Adds a plugin before `Target`.
    ///
    /// Existing instances are moved.
    ///
    /// # Panics
    /// Panics if `Target` does not exist.
    pub fn add_before<Target: Plugin>(self, plugin: impl Plugin) -> Self {
        self.try_add_before_overwrite::<Target, _>(plugin)
            .unwrap_or_else(|_| {
                panic!(
                    "Plugin does not exist in group: {}.",
                    core::any::type_name::<Target>()
                )
            })
    }

    /// Adds a plugin before `Target`, failing if it already exists.
    pub fn try_add_before<Target: Plugin, Insert: Plugin>(
        self,
        plugin: Insert,
    ) -> Result<Self, (Self, Insert)> {
        if self.contains::<Insert>() {
            return Err((self, plugin));
        }

        self.try_add_before_overwrite::<Target, _>(plugin)
    }

    /// Adds a plugin before `Target`, overwriting if it already exists.
    pub fn try_add_before_overwrite<Target: Plugin, Insert: Plugin>(
        mut self,
        plugin: Insert,
    ) -> Result<Self, (Self, Insert)> {
        let Some(target_index) = self.index_of::<Target>() else {
            return Err((self, plugin));
        };

        self.order.insert(target_index, TypeId::of::<Insert>());
        self.upsert_plugin_state(plugin, target_index);
        Ok(self)
    }

    /// Adds a plugin after `Target`.
    ///
    /// Existing instances are moved.
    ///
    /// # Panics
    /// Panics if `Target` does not exist.
    pub fn add_after<Target: Plugin>(self, plugin: impl Plugin) -> Self {
        self.try_add_after_overwrite::<Target, _>(plugin).unwrap_or_else(|_| {
            panic!(
                "Plugin does not exist in group: {}.",
                core::any::type_name::<Target>()
            )
        })
    }

    /// Adds a plugin after `Target`, failing if it already exists.
    pub fn try_add_after<Target: Plugin, Insert: Plugin>(
        self,
        plugin: Insert,
    ) -> Result<Self, (Self, Insert)> {
        if self.contains::<Insert>() {
            return Err((self, plugin));
        }

        self.try_add_after_overwrite::<Target, _>(plugin)
    }

    /// Adds a plugin after `Target`, overwriting if it already exists.
    pub fn try_add_after_overwrite<Target: Plugin, Insert: Plugin>(
        mut self,
        plugin: Insert,
    ) -> Result<Self, (Self, Insert)> {
        let Some(target_index) = self.index_of::<Target>() else {
            return Err((self, plugin));
        };

        let target_index = target_index + 1;
        self.order.insert(target_index, TypeId::of::<Insert>());
        self.upsert_plugin_state(plugin, target_index);
        Ok(self)
    }

    /// Enables a plugin previously disabled in this group.
    ///
    /// # Panics
    /// Panics if the plugin does not exist.
    pub fn enable<T: Plugin>(mut self) -> Self {
        let entry = self
            .plugins
            .get_mut(TypeId::of::<T>())
            .expect("Cannot enable a plugin that does not exist.");
        entry.enabled = true;
        self
    }

    /// Disables a plugin in this group.
    ///
    /// Disabled plugins still keep their relative order position.
    ///
    /// # Panics
    /// Panics if the plugin does not exist.
    pub fn disable<T: Plugin>(mut self) -> Self {
        let entry = self
            .plugins
            .get_mut(TypeId::of::<T>())
            .expect("Cannot disable a plugin that does not exist.");
        entry.enabled = false;
        self
    }

    /// Consumes the builder and adds all enabled plugins to the app.
    ///
    /// # Panics
    /// Panics if one plugin is already present in the application.
    #[track_caller]
    pub fn finish(mut self, app: &mut App) {
        for ty in &self.order {
            if let Some(entry) = self.plugins.remove(*ty)
                && entry.enabled
                && let Err(plugin) = app.add_boxed_plugin(entry.plugin)
            {
                let plugin_name = plugin.name();
                panic!(
                    "Error adding plugin {} in group {}: plugin was already added in application",
                    plugin_name, self.group_name,
                );
            }
        }
    }

    fn index_of<Target: Plugin>(&self) -> Option<usize> {
        self.order.iter().position(|&ty| ty == TypeId::of::<Target>())
    }

    fn upsert_plugin_state<T: Plugin>(&mut self, plugin: T, added_at_index: usize) {
        self.upsert_plugin_entry_state(
            TypeId::of::<T>(),
            PluginEntry {
                plugin: Box::new(plugin),
                enabled: true,
            },
            added_at_index,
        );
    }

    fn upsert_plugin_entry_state(
        &mut self,
        key: TypeId,
        plugin: PluginEntry,
        added_at_index: usize,
    ) {
        if let Some(entry) = self.plugins.insert(key, plugin) {
            if entry.enabled {
                log::warn!(
                    "You are replacing plugin '{}' that was not disabled.",
                    entry.plugin.name()
                );
            }
            if let Some(to_remove) = self
                .order
                .iter()
                .enumerate()
                .find(|(idx, ty)| *idx != added_at_index && **ty == key)
                .map(|(idx, _)| idx)
            {
                self.order.remove(to_remove);
            }
        }
    }
}

/// A plugin group that contains no plugins.
///
/// Useful for constructing minimal examples.
#[doc(hidden)]
pub struct NoopPluginGroup;

impl PluginGroup for NoopPluginGroup {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
    }
}

// -----------------------------------------------------------------------------
// Plugins

pub trait Plugins<Marker>: sealed::Plugins<Marker> {}

impl<Marker, T> Plugins<Marker> for T where T: sealed::Plugins<Marker> {}

mod sealed {
    use alloc::boxed::Box;

    use voker_utils::range_invoke2;

    use crate::App;
    use crate::plugin::{Plugin, PluginGroup};

    pub trait Plugins<Marker> {
        fn add_to_app(self, app: &mut App);
    }

    pub struct PluginMarker;
    pub struct PluginGroupMarker;
    pub struct PluginsTupleMarker;

    impl<P: Plugin> Plugins<PluginMarker> for P {
        #[track_caller]
        fn add_to_app(self, app: &mut App) {
            if let Err(plugins) = app.add_boxed_plugin(Box::new(self)) {
                let name = plugins.name();
                panic!("Error adding plugin {name}: plugin was already added in application");
            }
        }
    }

    impl<P: PluginGroup> Plugins<PluginGroupMarker> for P {
        #[track_caller]
        fn add_to_app(self, app: &mut App) {
            self.build().finish(app);
        }
    }

    macro_rules! impl_tuple_plugins {
        (0: []) => {
            impl Plugins<(PluginsTupleMarker,)> for () {
                fn add_to_app(self, _app: &mut App) {}
            }
        };
        (1 : [0 : P0 TO ]) => {
            #[cfg_attr(docsrs, doc(fake_variadic))]
            #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 8 items long.")]
            impl<P0, TO> Plugins<(PluginsTupleMarker, TO)> for (P0,)
            where
                P0: Plugins<T0>,
            {
                fn add_to_app(self, app: &mut App) {
                    self.0.add_to_app(app);
                }
            }
        };
        ($num:literal : [$($index:tt : $p:ident $m:ident),+]) => {
            #[cfg_attr(docsrs, doc(hidden))]
            impl<$($p, $m),*> Plugins<(PluginsTupleMarker, ($($m,)*),)> for ($($p,)*)
            where
                $($p: Plugins<$m>),*
            {
                fn add_to_app(self, app: &mut App) {
                   $( self.$index.add_to_app(app); )*
                }
            }
        };
    }

    range_invoke2!(impl_tuple_plugins, 8);
}
