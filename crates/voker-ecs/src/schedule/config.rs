//! System configuration builder for schedule insertion.
//!
//! `SystemConfig` is a temporary structure produced by [`IntoSystemConfig`]
//! implementations and consumed by [`crate::schedule::Schedule::config`].
//!
//! It captures:
//! - action/condition systems (primary members of the configured group),
//! - explicit ordering edges and run-condition edges,
//! - optional `ApplyDeferred` helpers for deferred systems,
//! - references to system-set boundaries used as ordering anchors.
//!
//! # Primary vs. Anchor systems
//!
//! `primary_ids` tracks the systems the user directly configured (for use with
//! `chain()`).  Systems introduced via `before()`/`after()` are registered in
//! `systems` but are **not** added to `primary_ids`, so they never participate
//! in chain ordering.
//!
//! Set boundaries referenced by `before_set()`/`after_set()` are stored as
//! `set_anchors` and are **not** inserted into `systems` at all; the schedule
//! resolves them when applying the config.
//!
//! # Deferred handling
//!
//! When a system `is_deferred()`, a paired `ApplyDeferred<S>` is created at
//! `into_config()` time (while the concrete type `S` is still known) and stored
//! in `deferred`.  Ordering methods use `deferred` to thread sync points into
//! the generated edges when `ignore_deferred = false`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use voker_utils::hash::{HashSet, NoopHashMap};

use crate::schedule::{ActionSystem, ConditionSystem};
use crate::schedule::{InternedSystemSet, SystemSet, apply_deferred};
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
    /// Set boundaries referenced for ordering but not registered here.
    pub(super) set_anchors: HashSet<InternedSystemSet>,
    /// Set membership for all primary systems.
    pub(super) set: Option<InternedSystemSet>,
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
            set_anchors: HashSet::new(),
            set: None,
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
        cfg.dependencies.insert((id, def_id));
        cfg
    }

    fn with_condition(system: ConditionSystem) -> Self {
        let id = system.id();
        let mut cfg = Self::new();
        cfg.systems.insert(id, SystemEntry::Condition(system));
        cfg.primary_ids.push(id);
        cfg
    }

    // ------------------------------------------------------------------
    // Helpers

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    #[inline]
    fn check_no_duplicate(&self, id: SystemId) {
        if self.systems.contains_key(&id) {
            let caller = DebugLocation::caller();
            log::error!("Duplicated system `{id}` in config, may cause a cycle. {caller}.");
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
            self.dependencies.insert((id, def_id));
        }

        // Intentionally NOT added to primary_ids.
        self
    }

    // ------------------------------------------------------------------
    // before_set / after_set implementations

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn before_set_impl(mut self, set: InternedSystemSet) -> Self {
        let begin_id = set.begin().id();
        for &pid in &self.primary_ids {
            self.dependencies.insert((pid, begin_id));
        }
        self.set_anchors.insert(set);
        self
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn after_set_impl(mut self, set: InternedSystemSet) -> Self {
        let end_id = set.end().id();
        for &pid in &self.primary_ids {
            self.dependencies.insert((end_id, pid));
        }
        self.set_anchors.insert(set);
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
                    log::error!("Duplicated system `{id}` in chain, may cause a cycle. {caller}.");
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

        self.systems.entry(id).or_insert(SystemEntry::Condition(condition));

        for &pid in &self.primary_ids {
            self.conditions.insert((id, pid));
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

        self.systems.entry(id).or_insert(SystemEntry::Action(action));

        // Treat action execution as a condition gate for all primaries.
        for &pid in &self.primary_ids {
            self.conditions.insert((id, pid));
        }
        self
    }

    // ------------------------------------------------------------------
    // in_set

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn in_set_impl(mut self, set: InternedSystemSet) -> Self {
        if let Some(old) = self.set {
            let caller = DebugLocation::caller();
            log::error!(
                "Duplicated in_set call: old `{old:?}`, new `{set:?}`, old is overwritten. \
                {caller}"
            );
        }
        self.set = Some(set);
        self
    }

    // ------------------------------------------------------------------
    // merge (used by tuple impls)

    fn merge(mut self, other: Self) -> Self {
        self.primary_ids.extend(other.primary_ids);
        self.systems.extend(other.systems);
        self.deferred.extend(other.deferred);
        self.dependencies.extend(other.dependencies);
        self.conditions.extend(other.conditions);
        self.set_anchors.extend(other.set_anchors);
        // set: prefer first non-None, warn if both are Some
        match (self.set, other.set) {
            (None, Some(s)) => self.set = Some(s),
            (Some(_), Some(s)) => {
                log::error!(
                    "Conflicting in_set when merging SystemConfig tuples (second `{s:?}` \
                    ignored). Use in_set on the tuple, not on individual members."
                );
            }
            _ => {}
        }
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

    /// Run self before the specified system set begins.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn before_set(self, s: impl SystemSet) -> SystemConfig {
        self.into_config().before_set_impl(s.intern())
    }

    /// Run self after the specified system set ends.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn after_set(self, s: impl SystemSet) -> SystemConfig {
        self.into_config().after_set_impl(s.intern())
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

    /// Register all primary systems as members of `set`.
    ///
    /// Each system should belong to exactly one set. Calling this more than once
    /// on the same config emits an error and overwrites the previous value.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn in_set(self, set: impl SystemSet) -> SystemConfig {
        self.into_config().in_set_impl(set.intern())
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
            let def: ActionSystem = Box::new(apply_deferred::<F>());
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
        SystemConfig::with_condition(condition)
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

    #[test]
    fn before_set_does_not_pollute_systems() {
        use crate::schedule::{AnonymousSystemSet, SystemSet};
        let set = AnonymousSystemSet;
        let begin_id = set.begin().id();
        let a_id: SystemId = sys_a.system_id();

        let config = sys_a.before_set(AnonymousSystemSet).into_config();
        assert!(config.dependencies.contains(&(a_id, begin_id)));
        // begin is NOT registered in systems — the schedule handles it
        assert!(!config.systems.contains_key(&begin_id));
        assert!(!config.primary_ids.contains(&begin_id));
        // The set is stored as an anchor
        assert!(config.set_anchors.contains(&set.intern()));
    }

    #[test]
    fn multiple_set_anchors_do_not_cross_pollute() {
        use crate::schedule::{AnonymousSystemSet, SystemSet};
        // This previously created an unintended end_SetC → begin_SetB edge.
        // Verify that composing before_set + after_set produces only intended edges.
        let a_id: SystemId = sys_a.system_id();
        let set = AnonymousSystemSet;
        let begin_id = set.begin().id();
        let end_id = set.end().id();

        // before_set and after_set on the same set as a simple smoke test
        let config = sys_a
            .before_set(AnonymousSystemSet)
            .after_set(AnonymousSystemSet)
            .into_config();

        assert!(config.dependencies.contains(&(a_id, begin_id)));
        assert!(config.dependencies.contains(&(end_id, a_id)));
        // No end → begin edge should exist
        assert!(!config.dependencies.contains(&(end_id, begin_id)));
    }
}
