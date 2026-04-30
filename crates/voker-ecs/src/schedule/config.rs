//! System configuration builder for schedule insertion.
//!
//! `SystemConfig` is a temporary structure produced by [`IntoSystemConfig`]
//! implementations and consumed by [`Schedule::add_systems`].
//!
//! It captures:
//! - action/condition systems (primary members of the configured group),
//! - explicit ordering edges and run-condition edges,
//! - optional `ApplyDeferred` helpers for deferred systems.
//!
//! # Primary vs. Anchor systems
//!
//! `primary_ids` tracks the systems the user directly configured (for use with
//! `chain()`).  Systems introduced via `before()`/`after()` are registered in
//! `systems` but are **not** added to `primary_ids`, so they never participate
//! in chain ordering.
//!
//! Cross-set ordering is not expressible here; use [`Schedule::config_set`]
//! and [`IntoSystemSetConfig`] for that.
//!
//! [`Schedule::add_systems`]: crate::schedule::Schedule::add_systems
//! [`Schedule::config_set`]: crate::schedule::Schedule::config_set
//! [`IntoSystemSetConfig`]: crate::schedule::IntoSystemSetConfig
//!
//! # Deferred handling
//!
//! When a system `is_deferred()`, a paired `ApplyDeferred<S>` is created at
//! `into_config()` time (while the concrete type `S` is still known) and stored
//! in `deferred`.  Ordering methods use `deferred` to thread sync points into
//! the generated edges when `ignore_deferred = false`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use voker_utils::hash::{HashMap, HashSet, NoopHashMap};

use crate::schedule::apply_deferred;
use crate::schedule::{ActionSystem, ConditionSystem};
use crate::system::InternedSystemSet;
use crate::system::{IntoSystem, System, SystemId};
use crate::utils::DebugLocation;

// -----------------------------------------------------------------------------
// SystemEntry

pub(super) enum SystemEntry {
    Action(ActionSystem),
    Condition(ConditionSystem),
}

// -----------------------------------------------------------------------------
// SystemConfig

#[derive(Default)]
pub struct SystemConfig {
    /// Systems directly configured by the user; determines chain() order.
    pub(super) primary_ids: Vec<SystemId>,
    /// All systems to be registered (primaries + anchors from before/after).
    pub(super) systems: NoopHashMap<SystemId, SystemEntry>,
    /// `ApplyDeferred` helpers, keyed by the primary system they follow.
    pub(super) deferred: NoopHashMap<SystemId, ActionSystem>,
    /// Ordering edges: `(before, after)`.
    pub(super) dependencies: HashSet<(SystemId, SystemId)>,
    /// Condition edges: `(condition, gated)`.
    pub(super) conditions: HashSet<(SystemId, SystemId)>,
}

impl SystemConfig {
    #[inline]
    const fn new() -> Self {
        Self {
            primary_ids: Vec::new(),
            systems: NoopHashMap::new(),
            deferred: NoopHashMap::new(),
            dependencies: HashSet::new(),
            conditions: HashSet::new(),
        }
    }

    fn with_action(system: ActionSystem) -> Self {
        let id = system.id();
        let mut cfg = Self::new();
        cfg.systems.insert(id, SystemEntry::Action(system));
        cfg.primary_ids.push(id);
        cfg
    }

    fn with_action_deferred(system: ActionSystem, def: ActionSystem) -> Self {
        let id = system.id();
        let def_id = def.id();
        let mut cfg = Self::new();
        cfg.systems.insert(id, SystemEntry::Action(system));
        cfg.primary_ids.push(id);
        cfg.deferred.insert(id, def);
        cfg.conditions.insert((id, def_id)); // run if ran optimization
        cfg
    }

    fn with_condition(system: ConditionSystem) -> Self {
        let id = system.id();
        let mut cfg = Self::new();
        cfg.systems.insert(id, SystemEntry::Condition(system));
        cfg.primary_ids.push(id);
        cfg
    }

    fn with_condition_deferred(system: ConditionSystem, def: ActionSystem) -> Self {
        let id = system.id();
        let mut cfg = Self::new();
        cfg.systems.insert(id, SystemEntry::Condition(system));
        cfg.primary_ids.push(id);
        cfg.deferred.insert(id, def);
        // No deps are required here, `add_systems` will handle it.
        cfg
    }

    // ------------------------------------------------------------------
    // Helpers

    #[inline]
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn check_no_duplicate(&self, id: SystemId) {
        if self.systems.contains_key(&id) {
            let caller = DebugLocation::caller();
            tracing::error!("Duplicated system `{id}` in config, may cause a cycle. {caller}.");
        }
    }

    // ------------------------------------------------------------------
    // before / after implementations

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn before_impl<S, M>(mut self, system: S, ignore_deferred: bool) -> Self
    where
        S: IntoSystem<(), (), M>,
    {
        let action: ActionSystem = Box::new(IntoSystem::into_system(system));
        let id = action.id();

        self.check_no_duplicate(id);

        self.systems.entry(id).or_insert(SystemEntry::Action(action));

        // Edges: each primary (and its deferred if applicable) → new anchor.
        for &pid in &self.primary_ids {
            self.dependencies.insert((pid, id));
            if !ignore_deferred && let Some(def) = self.deferred.get(&pid) {
                self.dependencies.insert((def.id(), id));
            }
        }
        // Intentionally NOT added to primary_ids — anchors never chain.
        self
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn after_impl<S, M>(mut self, system: S, ignore_deferred: bool) -> Self
    where
        S: IntoSystem<(), (), M>,
    {
        let action: ActionSystem = Box::new(IntoSystem::into_system(system));
        let id = action.id();
        // If anchor is deferred and !ignore_deferred, create its sync point so
        // primaries observe flushed commands: anchor → anchor_deferred → primaries.
        let anchor_def: Option<ActionSystem> = if !ignore_deferred && action.is_deferred() {
            Some(Box::new(apply_deferred::<S>()))
        } else {
            None
        };

        self.check_no_duplicate(id);

        self.systems.entry(id).or_insert(SystemEntry::Action(action));

        // Edge: new anchor (or its deferred) → each primary.
        let before_primaries = anchor_def.as_ref().map(|d| d.id()).unwrap_or(id);
        for &pid in &self.primary_ids {
            self.dependencies.insert((before_primaries, pid));
        }

        if let Some(def) = anchor_def {
            let def_id = def.id();
            self.systems.entry(def_id).or_insert(SystemEntry::Action(def));
        }

        // Intentionally NOT added to primary_ids.
        self
    }

    // ------------------------------------------------------------------
    // chain implementation

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn chain_impl(mut self, ignore_deferred: bool) -> Self {
        #[cfg(any(debug_assertions, feature = "debug"))]
        {
            let mut seen = HashSet::new();
            for &id in &self.primary_ids {
                if !seen.insert(id) {
                    let caller = DebugLocation::caller();
                    tracing::error!(
                        "Duplicated system `{id}` in chain, may cause a cycle. {caller}."
                    );
                }
            }
        }

        // Index-based loop: `SystemId` is Copy, so each iteration reads two
        // values out of the slice without holding a live slice reference across
        // the loop body, allowing `self.deferred` / `self.dependencies` to be
        // accessed separately without cloning `primary_ids`.
        for i in 0..self.primary_ids.len().saturating_sub(1) {
            let a = self.primary_ids[i];
            let b = self.primary_ids[i + 1];
            if !ignore_deferred && let Some(def) = self.deferred.get(&a) {
                let def_id = def.id();
                self.dependencies.insert((a, def_id));
                self.dependencies.insert((def_id, b));
                continue;
            }
            self.dependencies.insert((a, b));
        }
        self
    }

    // ------------------------------------------------------------------
    // run_if / run_if_ran implementations

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn run_if_impl<S, M>(mut self, system: S) -> Self
    where
        S: IntoSystem<(), bool, M>,
    {
        let condition: ConditionSystem = Box::new(IntoSystem::into_system(system));
        let id = condition.id();

        self.check_no_duplicate(id);

        let def: Option<ActionSystem> = if condition.is_deferred() {
            Some(Box::new(apply_deferred::<S::System>()))
        } else {
            None
        };

        self.systems.entry(id).or_insert(SystemEntry::Condition(condition));

        for &pid in &self.primary_ids {
            self.conditions.insert((id, pid));
        }

        if let Some(def) = def {
            let def_id = def.id();
            self.deferred.insert(id, def);

            for &pid in &self.primary_ids {
                self.dependencies.insert((def_id, pid));
            }
        }

        // Condition is not a primary.
        self
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn run_if_ran_impl<S, M>(mut self, system: S) -> Self
    where
        S: IntoSystem<(), (), M>,
    {
        let action: ActionSystem = Box::new(IntoSystem::into_system(system));
        let id = action.id();

        self.check_no_duplicate(id);

        let def: Option<ActionSystem> = if action.is_deferred() {
            Some(Box::new(apply_deferred::<S::System>()))
        } else {
            None
        };

        self.systems.entry(id).or_insert(SystemEntry::Action(action));

        // Treat action execution as a condition gate for all primaries.
        for &pid in &self.primary_ids {
            self.conditions.insert((id, pid));
        }

        if let Some(def) = def {
            let def_id = def.id();
            self.deferred.insert(id, def);

            for &pid in &self.primary_ids {
                self.dependencies.insert((def_id, pid));
            }
        }

        self
    }

    // ------------------------------------------------------------------
    // apply_to_set

    /// Updates all systems in the config so their [`SystemId`] contains `set`,
    /// then rekeys all internal maps and edge sets to match the new IDs.
    ///
    /// This is called by [`Schedule::add_systems`] before inserting systems so
    /// each system registered under a given set has a unique identity that
    /// encodes set membership.
    pub(super) fn apply_to_set(&mut self, set: InternedSystemSet) {
        let mut id_map: HashMap<SystemId, SystemId> = HashMap::new();

        // Remap all systems (primaries + ordering anchors from before/after).
        let old_systems: NoopHashMap<SystemId, SystemEntry> = core::mem::take(&mut self.systems);
        for (old_id, mut entry) in old_systems {
            match &mut entry {
                SystemEntry::Action(sys) => sys.set_system_set(set),
                SystemEntry::Condition(sys) => sys.set_system_set(set),
            }
            let new_id = match &entry {
                SystemEntry::Action(sys) => sys.id(),
                SystemEntry::Condition(sys) => sys.id(),
            };
            id_map.insert(old_id, new_id);
            self.systems.insert(new_id, entry);
        }

        // Remap deferred helpers: keyed by primary system's old ID.
        let old_deferred: NoopHashMap<SystemId, ActionSystem> = core::mem::take(&mut self.deferred);
        for (old_primary_id, mut def_sys) in old_deferred {
            let old_def_id = def_sys.id();
            def_sys.set_system_set(set);
            let new_def_id = def_sys.id();
            id_map.insert(old_def_id, new_def_id);
            let new_primary_id = id_map.get(&old_primary_id).copied().unwrap_or(old_primary_id);
            self.deferred.insert(new_primary_id, def_sys);
        }

        // Remap primary_ids.
        for id in &mut self.primary_ids {
            *id = id_map.get(id).copied().unwrap_or(*id);
        }

        // Remap dependency edges.
        let old_deps: HashSet<(SystemId, SystemId)> = core::mem::take(&mut self.dependencies);
        for (a, b) in old_deps {
            let new_a = id_map.get(&a).copied().unwrap_or(a);
            let new_b = id_map.get(&b).copied().unwrap_or(b);
            self.dependencies.insert((new_a, new_b));
        }

        // Remap condition edges.
        let old_conds: HashSet<(SystemId, SystemId)> = core::mem::take(&mut self.conditions);
        for (cond, runnable) in old_conds {
            let new_cond = id_map.get(&cond).copied().unwrap_or(cond);
            let new_runnable = id_map.get(&runnable).copied().unwrap_or(runnable);
            self.conditions.insert((new_cond, new_runnable));
        }
    }

    // ------------------------------------------------------------------
    // merge (used by tuple impls)

    fn merge(mut self, other: Self) -> Self {
        self.primary_ids.extend(other.primary_ids);
        self.systems.extend(other.systems);
        self.deferred.extend(other.deferred);
        self.dependencies.extend(other.dependencies);
        self.conditions.extend(other.conditions);
        self
    }
}

// -----------------------------------------------------------------------------
// IntoSystemConfig

/// Converts a value into a [`SystemConfig`] that can be inserted into a schedule.
///
/// Implemented for:
/// - action systems (`IntoSystem<(), (), _>`),
/// - condition systems (`IntoSystem<(), bool, _>`),
/// - tuples (up to 8 elements) of other `IntoSystemConfig` values,
/// - [`SystemConfig`] itself.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not describe a valid system configuration",
    label = "invalid system configuration"
)]
pub trait IntoSystemConfig<Marker>: Sized {
    fn into_config(self) -> SystemConfig;

    /// Run self **before** `s`. Both are registered; `s` is not a primary.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn before<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().before_impl(s, false)
    }

    /// Run self **after** `s`. Both are registered; `s` is not a primary.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn after<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().after_impl(s, false)
    }

    /// Add sequential ordering between adjacent primary systems.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn chain(self) -> SystemConfig {
        self.into_config().chain_impl(false)
    }

    /// Add a `bool`-returning condition that gates all primary systems.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn run_if<M>(self, c: impl IntoSystem<(), bool, M>) -> SystemConfig {
        self.into_config().run_if_impl(c)
    }

    /// Gate all primary systems on whether `s` successfully ran this frame.
    ///
    /// Unlike [`run_if`](Self::run_if), `s` is a `()` action system; its
    /// execution acts as a condition edge (runs → primaries run).
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn run_if_ran<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().run_if_ran_impl(s)
    }

    /// Like [`before`](Self::before) but omits deferred sync edges.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn before_ignore_deferred<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().before_impl(s, true)
    }

    /// Like [`after`](Self::after) but omits deferred sync edges.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn after_ignore_deferred<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().after_impl(s, true)
    }

    /// Like [`chain`](Self::chain) but omits deferred sync edges.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn chain_ignore_deferred(self) -> SystemConfig {
        self.into_config().chain_impl(true)
    }
}

// -----------------------------------------------------------------------------
// IntoSystemConfig implementations

impl IntoSystemConfig<()> for SystemConfig {
    #[inline(always)]
    fn into_config(self) -> Self {
        self
    }
}

impl IntoSystemConfig<()> for () {
    #[inline]
    fn into_config(self) -> SystemConfig {
        SystemConfig::new()
    }
}

impl IntoSystemConfig<()> for Box<dyn System<Input = (), Output = ()>> {
    #[inline]
    fn into_config(self) -> SystemConfig {
        SystemConfig::with_action(self)
    }
}

impl<F, M> IntoSystemConfig<(fn(), M)> for F
where
    F: IntoSystem<(), (), M>,
{
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn into_config(self) -> SystemConfig {
        let action: ActionSystem = Box::new(IntoSystem::into_system(self));
        if action.is_deferred() {
            let def: ActionSystem = Box::new(apply_deferred::<F::System>());
            SystemConfig::with_action_deferred(action, def)
        } else {
            SystemConfig::with_action(action)
        }
    }
}

impl<F, M> IntoSystemConfig<(fn() -> bool, M)> for F
where
    F: IntoSystem<(), bool, M>,
{
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn into_config(self) -> SystemConfig {
        let condition: ConditionSystem = Box::new(IntoSystem::into_system(self));
        if condition.is_deferred() {
            let def: ActionSystem = Box::new(apply_deferred::<F::System>());
            SystemConfig::with_condition_deferred(condition, def)
        } else {
            SystemConfig::with_condition(condition)
        }
    }
}

macro_rules! impl_tuple_into_system_config {
    (0: []) => {};
    (1 : [0 : P0 TO ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 8 items long.")]
        impl<P, M> IntoSystemConfig<(SystemConfig, (P, M),)> for (P,)
        where
            P: IntoSystemConfig<M>
        {
            #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
            fn into_config(self) -> SystemConfig {
                self.0.into_config()
            }
        }
    };
    ($num:literal : [$($index:tt : $p:ident $m:ident),+]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        impl<$($p, $m),*> IntoSystemConfig<(SystemConfig, ($($m,)*),)> for ($($p,)*)
        where
            $($p: IntoSystemConfig<$m>),*
        {
            #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
            fn into_config(self) -> SystemConfig {
                SystemConfig::default() $(.merge(self.$index.into_config()))*
            }
        }
    };
}

voker_utils::range_invoke2!(impl_tuple_into_system_config, 8);

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::IntoSystemConfig;
    use crate::system::{IntoSystem, SystemId};

    fn sys_a() {}
    fn sys_b() {}
    fn sys_c() {}
    fn cond_true() -> bool {
        true
    }

    #[test]
    fn before_adds_primary_to_anchor_dependency() {
        let a_id: SystemId = sys_a.system_id();
        let b_id: SystemId = sys_b.system_id();

        let config = sys_a.before(sys_b).into_config();
        assert!(config.dependencies.contains(&(a_id, b_id)));
        // b is NOT a primary
        assert_eq!(config.primary_ids, vec![a_id]);
    }

    #[test]
    fn after_adds_anchor_to_primary_dependency() {
        let a_id: SystemId = sys_a.system_id();
        let b_id: SystemId = sys_b.system_id();

        let config = sys_a.after(sys_b).into_config();
        assert!(config.dependencies.contains(&(b_id, a_id)));
        assert_eq!(config.primary_ids, vec![a_id]);
    }

    #[test]
    fn chain_adds_linear_dependencies() {
        let a_id: SystemId = sys_a.system_id();
        let b_id: SystemId = sys_b.system_id();
        let c_id: SystemId = sys_c.system_id();

        let config = (sys_a, sys_b, sys_c).chain().into_config();
        assert!(config.dependencies.contains(&(a_id, b_id)));
        assert!(config.dependencies.contains(&(b_id, c_id)));
        // All three are primaries
        assert_eq!(config.primary_ids, vec![a_id, b_id, c_id]);
    }

    #[test]
    fn chain_does_not_include_before_anchors() {
        let a_id: SystemId = sys_a.system_id();
        let b_id: SystemId = sys_b.system_id();
        let c_id: SystemId = sys_c.system_id();

        // sys_c is an anchor, not a primary. chain() should only chain a→b.
        let config = (sys_a, sys_b).before(sys_c).chain().into_config();
        assert!(config.dependencies.contains(&(a_id, b_id)));
        assert!(config.dependencies.contains(&(a_id, c_id)));
        assert!(config.dependencies.contains(&(b_id, c_id)));
        // c is NOT in primary_ids
        assert!(!config.primary_ids.contains(&c_id));
    }

    #[test]
    fn run_if_adds_condition_edge() {
        let cond_id: SystemId = cond_true.system_id();
        let a_id: SystemId = sys_a.system_id();

        let config = sys_a.run_if(cond_true).into_config();
        assert!(config.conditions.contains(&(cond_id, a_id)));
    }
}
