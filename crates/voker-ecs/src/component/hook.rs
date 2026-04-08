use super::ComponentId;
use crate::entity::Entity;
use crate::utils::DebugLocation;
use crate::world::DeferredWorld;

/// The type used for [`Component`] lifecycle hooks
/// such as `on_add`, `on_insert` or `on_remove`.
///
/// [`Component`]: super::Component
pub type ComponentHook = fn(DeferredWorld, HookContext);

/// Context provided to a [`ComponentHook`].
#[derive(Debug, Clone, Copy)]
pub struct HookContext {
    pub id: ComponentId,
    pub entity: Entity,
    pub caller: DebugLocation,
}

/// [`World`]-mutating functions that run as part of lifecycle events of a [`Component`].
///
/// Hooks are functions that run when a component is added, overwritten, or removed from
/// an entity. These are intended to be used for structural side effects that need to
/// happen when a component is added or removed, and are not intended for general-purpose logic.
///
/// # Operator & Hook
///
/// - Entity spawn: `on_add -> on_insert`
/// - Entity despawn: `on_despawn -> on_discard -> on_remove`
/// - Component insert: `on_discard` (replaced) -> `on_add` (new) -> `on_insert`
/// - Component remove: `on_discard -> on_remove`
/// - Entity clear: `on_discard -> on_remove`
/// - Entity clone: `on_clone -> on_add -> on_insert`
///
/// # Trigger Timing
///
/// Lifecycle hook execution requires exclusive world access, so hooks are
/// invoked immediately when their associated operation is processed.
///
/// To prevent hooks from directly reshaping world structure, the first
/// parameter of [`ComponentHook`] is [`DeferredWorld`]. This allows data reads
/// and writes, while structural mutations are routed through deferred commands.
///
/// Unlike hooks themselves, deferred commands do not execute immediately.
///
/// For all operations listed above, deferred commands enqueued by hooks are
/// applied after the operation's core logic completes.
///
/// Therefore, deferred hook logic should be robust against state changes.
/// For example, a deferred command queued from `on_discard` must assume the
/// discarded component has already been consumed when that command runs.
///
/// [`Component`]: crate::component::Component
/// [`World`]: crate::world::World
#[derive(Debug, Clone)]
pub struct ComponentHooks {
    pub(super) on_add: Option<ComponentHook>,
    pub(super) on_clone: Option<ComponentHook>,
    pub(super) on_insert: Option<ComponentHook>,
    pub(super) on_remove: Option<ComponentHook>,
    pub(super) on_discard: Option<ComponentHook>,
    pub(super) on_despawn: Option<ComponentHook>,
}

impl ComponentHooks {
    /// Register a [`ComponentHook`] that will be run when this component is added to an entity.
    ///
    /// An `on_add` hook will always run before `on_insert` hooks.
    /// Spawning an entity counts as adding all of its components.
    ///
    /// # Panics
    ///
    /// Will panic if the component already has an `on_add` hook
    pub fn on_add(&mut self, hook: ComponentHook) -> &mut Self {
        self.try_on_add(hook).expect("Component already has an on_add hook")
    }

    /// Register a [`ComponentHook`] that will be run when this component is added (with `.insert`)
    /// or replaced.
    ///
    /// An `on_clone` hook always runs before any `on_add` and `on_insert` hooks(if the entity was cloned).
    ///
    /// # Warning
    ///
    /// The hook won't run if the component is already present and is only mutated, such
    /// as in a system via a query. As a result, this needs to be combined with immutable
    /// components to serve as a mechanism for reliably updating indexes and other caches.
    ///
    /// # Panics
    ///
    /// Will panic if the component already has an `on_clone` hook
    pub fn on_clone(&mut self, hook: ComponentHook) -> &mut Self {
        self.try_on_clone(hook)
            .expect("Component already has an on_clone hook")
    }

    /// Register a [`ComponentHook`] that will be run when this component is added (with `.insert`)
    /// or replaced.
    ///
    /// An `on_insert` hook always runs after any `on_add` hooks
    /// (if the entity didn't already have the component).
    ///
    /// # Warning
    ///
    /// The hook won't run if the component is already present and is only mutated, such
    /// as in a system via a query. As a result, this needs to be combined with immutable
    /// components to serve as a mechanism for reliably updating indexes and other caches.
    ///
    /// # Panics
    ///
    /// Will panic if the component already has an `on_insert` hook
    pub fn on_insert(&mut self, hook: ComponentHook) -> &mut Self {
        self.try_on_insert(hook)
            .expect("Component already has an on_insert hook")
    }

    /// Register a [`ComponentHook`] that will be run when this component is about to be dropped,
    /// such as being replaced (with `.insert`) or removed.
    ///
    /// If this component is inserted onto an entity that already has it, this hook will run
    /// before the value is replaced, allowing access to the previous data just before it is
    /// dropped. This hook does *not* run if the entity did not already have this component.
    ///
    /// An `on_discard` hook always runs before any `on_remove` hooks, and always runs after
    /// any `on_despawn` hooks,
    ///
    /// # Warning
    ///
    /// The hook won't run if the component is already present and is only mutated, such as in
    /// a system via a query. As a result, this needs to be combined with immutable components
    /// to serve as a mechanism for reliably updating indexes and other caches.
    ///
    /// # Panics
    ///
    /// Will panic if the component already has an `on_discard` hook
    pub fn on_discard(&mut self, hook: ComponentHook) -> &mut Self {
        self.try_on_discard(hook)
            .expect("Component already has an on_discard hook")
    }

    /// Register a [`ComponentHook`] that will be run when this component is removed from an entity.
    /// Despawning an entity counts as removing all of its components.
    ///
    /// An `on_remove` hook always runs **after** any `on_remove` and `on_despawn` hooks (if exist).
    ///
    /// # Panics
    ///
    /// Will panic if the component already has an `on_remove` hook
    pub fn on_remove(&mut self, hook: ComponentHook) -> &mut Self {
        self.try_on_remove(hook)
            .expect("Component already has an on_remove hook")
    }

    /// Register a [`ComponentHook`] that will be run for each component on an entity when it is despawned.
    ///
    /// An `on_despawn` hook always runs **before** any `on_remove` and `on_despawn` hooks (if exist).
    ///
    /// # Panics
    ///
    /// Will panic if the component already has an `on_despawn` hook
    pub fn on_despawn(&mut self, hook: ComponentHook) -> &mut Self {
        self.try_on_despawn(hook)
            .expect("Component already has an on_despawn hook")
    }

    /// Attempt to register a [`ComponentHook`] that will be run when this component is added to an entity.
    ///
    /// This is a fallible version of [`Self::on_add`].
    ///
    /// Returns `None` if the component already has an `on_add` hook.
    pub fn try_on_add(&mut self, hook: ComponentHook) -> Option<&mut Self> {
        if self.on_add.is_some() {
            return None;
        }
        self.on_add = Some(hook);
        Some(self)
    }

    /// Attempt to register a [`ComponentHook`] that will be run when this component is cloned to an new entity.
    ///
    /// This is a fallible version of [`Self::on_clone`].
    ///
    /// Returns `None` if the component already has an `on_clone` hook.
    pub fn try_on_clone(&mut self, hook: ComponentHook) -> Option<&mut Self> {
        if self.on_clone.is_some() {
            return None;
        }
        self.on_clone = Some(hook);
        Some(self)
    }

    /// Attempt to register a [`ComponentHook`] that will be run when this component is added (with `.insert`)
    ///
    /// This is a fallible version of [`Self::on_insert`].
    ///
    /// Returns `None` if the component already has an `on_insert` hook.
    pub fn try_on_insert(&mut self, hook: ComponentHook) -> Option<&mut Self> {
        if self.on_insert.is_some() {
            return None;
        }
        self.on_insert = Some(hook);
        Some(self)
    }

    /// Attempt to register a [`ComponentHook`] that will be run when this component is replaced (with `.insert`) or removed
    ///
    /// This is a fallible version of [`Self::on_discard`].
    ///
    /// Returns `None` if the component already has an `on_discard` hook.
    pub fn try_on_discard(&mut self, hook: ComponentHook) -> Option<&mut Self> {
        if self.on_discard.is_some() {
            return None;
        }
        self.on_discard = Some(hook);
        Some(self)
    }

    /// Attempt to register a [`ComponentHook`] that will be run when this component is removed from an entity.
    ///
    /// This is a fallible version of [`Self::on_remove`].
    ///
    /// Returns `None` if the component already has an `on_remove` hook.
    pub fn try_on_remove(&mut self, hook: ComponentHook) -> Option<&mut Self> {
        if self.on_remove.is_some() {
            return None;
        }
        self.on_remove = Some(hook);
        Some(self)
    }

    /// Attempt to register a [`ComponentHook`] that will be run for each component on an entity when it is despawned.
    ///
    /// This is a fallible version of [`Self::on_despawn`].
    ///
    /// Returns `None` if the component already has an `on_despawn` hook.
    pub fn try_on_despawn(&mut self, hook: ComponentHook) -> Option<&mut Self> {
        if self.on_despawn.is_some() {
            return None;
        }
        self.on_despawn = Some(hook);
        Some(self)
    }
}
