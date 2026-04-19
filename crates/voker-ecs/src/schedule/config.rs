//! System configuration builder for schedule insertion.
//!
//! `SystemConfig` is a temporary structure produced by [`IntoSystemConfig`]
//! implementations and consumed by [`crate::schedule::Schedule::config`].
//!
//! It captures:
//! - action/condition systems,
//! - explicit ordering edges,
//! - condition edges,
//! - optional `ApplyDeferred` helper systems inserted for deferred actions.
//!
//! Deferred handling is controlled by method variants:
//! - `before` / `after` / `chain` keep deferred synchronization edges.
//! - `before_ignore_deferred` / `after_ignore_deferred` /
//!   `chain_ignore_deferred` skip extra deferred ordering edges.

use alloc::collections::VecDeque;

use alloc::boxed::Box;
use voker_utils::hash::map::Entry;
use voker_utils::hash::{HashSet, NoOpHashMap};

use crate::schedule::{
    ActionSystem, ConditionSystem, InternedScheduleSet, SystemSet, apply_deferred,
};
use crate::system::{IntoSystem, SystemId};
use crate::utils::DebugLocation;

pub(super) enum SystemNode {
    Action(ActionSystem),
    Condition(ConditionSystem),
}

#[derive(Default)]
pub struct SystemConfig {
    pub(super) idents: VecDeque<SystemId>,
    pub(super) system_set: Option<InternedScheduleSet>,
    pub(super) systems: NoOpHashMap<SystemId, SystemNode>,
    pub(super) deferred: NoOpHashMap<SystemId, ActionSystem>,
    pub(super) dependency: HashSet<(SystemId, SystemId)>,
    pub(super) condition: HashSet<(SystemId, SystemId)>,
}

impl SystemConfig {
    #[inline]
    const fn new() -> Self {
        Self {
            idents: VecDeque::new(),
            system_set: None,
            systems: NoOpHashMap::new(),
            deferred: NoOpHashMap::new(),
            dependency: HashSet::new(),
            condition: HashSet::new(),
        }
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn before_impl<S, M>(mut self, system: S, ignore_deferred: bool) -> Self
    where
        S: IntoSystem<(), (), M>,
    {
        let id: SystemId = system.system_id();

        let entry = self.systems.entry(id);

        if matches!(&entry, Entry::Occupied(_)) {
            let caller = DebugLocation::caller();
            log::error!("Duplicated systems `{id}`, may cause an infinite loop. {caller}.")
        }

        let mut deferred: Option<ActionSystem> = None;

        let action: ActionSystem = Box::new(IntoSystem::into_system(system));
        if action.is_deferred() {
            deferred = Some(Box::new(apply_deferred::<S>()))
        }

        entry.insert(SystemNode::Action(action));

        #[inline(never)]
        fn before_internal(
            this: &mut SystemConfig,
            id: SystemId,
            deferred: Option<ActionSystem>,
            ignore_deferred: bool,
        ) {
            this.idents.push_back(id);

            for &before in this.systems.keys() {
                if before != id {
                    this.dependency.insert((before, id));
                }
            }

            if !ignore_deferred {
                for &before in this.deferred.keys() {
                    if before != id {
                        this.dependency.insert((before, id));
                    }
                }
            }

            if let Some(deferred) = deferred {
                let deferred_id = deferred.id();
                this.idents.push_back(deferred_id);
                this.deferred.insert(deferred_id, deferred);
                this.dependency.insert((id, deferred_id));
                this.dependency.remove(&(deferred_id, id));
            }
        }

        before_internal(&mut self, id, deferred, ignore_deferred);

        self
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn after_impl<S, M>(mut self, system: S, ignore_deferred: bool) -> Self
    where
        S: IntoSystem<(), (), M>,
    {
        let id: SystemId = system.system_id();

        let entry = self.systems.entry(id);

        if matches!(&entry, Entry::Occupied(_)) {
            let caller = DebugLocation::caller();
            log::error!("Duplicated systems `{id}`, may cause an infinite loop. {caller}.")
        }

        let mut deferred: Option<ActionSystem> = None;

        let action: ActionSystem = Box::new(IntoSystem::into_system(system));
        if action.is_deferred() {
            deferred = Some(Box::new(apply_deferred::<S>()))
        }

        entry.insert(SystemNode::Action(action));

        #[inline(never)]
        fn after_internal(
            this: &mut SystemConfig,
            id: SystemId,
            deferred: Option<ActionSystem>,
            ignore_deferred: bool,
        ) {
            for &after in this.systems.keys() {
                if after != id {
                    this.dependency.insert((id, after));
                }
            }

            if let Some(deferred) = deferred {
                let deferred_id = deferred.id();
                this.idents.push_front(deferred_id);
                this.deferred.insert(deferred_id, deferred);
                this.dependency.insert((id, deferred_id));
                this.dependency.remove(&(deferred_id, id));

                if !ignore_deferred {
                    for &after in this.systems.keys() {
                        if after != deferred_id && after != id {
                            this.dependency.insert((deferred_id, after));
                        }
                    }
                }
            }

            this.idents.push_front(id);
        }

        after_internal(&mut self, id, deferred, ignore_deferred);

        self
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn chain_impl(mut self, ignore_deferred: bool) -> Self {
        let mut iter = self.idents.iter();

        while let Some(before) = iter.next() {
            let mut afters = iter.clone();
            if afters.any(|after| *after == *before) {
                let caller = DebugLocation::caller();
                log::error!("Duplicated systems `{before}`, may cause an infinite loop. {caller}.")
            }
        }

        if !ignore_deferred {
            let mut iter = self.idents.iter();
            while let Some(&before) = iter.next() {
                if let Some(&after) = iter.clone().next() {
                    self.dependency.insert((before, after));
                }
            }
            return self;
        }

        let mut iter = self.idents.iter();
        while let Some(before) = iter.next() {
            if !self.systems.contains_key(before) {
                continue;
            }
            for after in iter.clone() {
                if self.systems.contains_key(before) {
                    self.dependency.insert((*before, *after));
                    break;
                }
            }
        }
        self
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn run_if_impl<S, M>(mut self, system: S) -> Self
    where
        S: IntoSystem<(), bool, M>,
    {
        let id: SystemId = system.system_id();

        let entry = self.systems.entry(id);

        if matches!(&entry, Entry::Occupied(_)) {
            let caller = DebugLocation::caller();
            log::error!("Duplicated systems `{id}`, may cause an infinite loop. {caller}.")
        }

        let mut deferred: Option<ActionSystem> = None;

        let condition: ConditionSystem = Box::new(IntoSystem::into_system(system));
        if condition.is_deferred() {
            deferred = Some(Box::new(apply_deferred::<S>()))
        }

        entry.insert(SystemNode::Condition(condition));

        #[inline(never)]
        fn run_if_internal(this: &mut SystemConfig, id: SystemId, deferred: Option<ActionSystem>) {
            for &after in this.systems.keys() {
                if after != id {
                    this.condition.insert((id, after));
                }
            }

            if let Some(deferred) = deferred {
                let deferred_id = deferred.id();
                this.idents.push_front(deferred_id);
                this.deferred.insert(deferred_id, deferred);
                this.dependency.insert((id, deferred_id));
                this.dependency.remove(&(deferred_id, id));

                for &after in this.systems.keys() {
                    if after != deferred_id && after != id {
                        this.dependency.insert((deferred_id, after));
                    }
                }
            }

            this.idents.push_front(id);
        }

        run_if_internal(&mut self, id, deferred);

        self
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn run_if_run_impl<S, M>(mut self, system: S) -> Self
    where
        S: IntoSystem<(), (), M>,
    {
        let id: SystemId = system.system_id();

        let entry = self.systems.entry(id);

        if matches!(&entry, Entry::Occupied(_)) {
            let caller = DebugLocation::caller();
            log::error!("Duplicated systems `{id}`, may cause an infinite loop {caller}.")
        }

        let mut deferred: Option<ActionSystem> = None;

        let action: ActionSystem = Box::new(IntoSystem::into_system(system));
        if action.is_deferred() {
            deferred = Some(Box::new(apply_deferred::<S>()))
        }

        entry.insert(SystemNode::Action(action));

        #[inline(never)]
        fn run_if_run_internal(
            this: &mut SystemConfig,
            id: SystemId,
            deferred: Option<ActionSystem>,
        ) {
            for &after in this.systems.keys() {
                if after != id {
                    this.condition.insert((id, after));
                }
            }

            if let Some(deferred) = deferred {
                let deferred_id = deferred.id();
                this.idents.push_front(deferred_id);
                this.deferred.insert(deferred_id, deferred);
                this.dependency.insert((id, deferred_id));
                this.dependency.remove(&(deferred_id, id));

                for &after in this.systems.keys() {
                    if after != deferred_id && after != id {
                        this.dependency.insert((deferred_id, after));
                    }
                }
            }

            this.idents.push_front(id);
        }

        run_if_run_internal(&mut self, id, deferred);

        self
    }

    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn in_set_impl(mut self, set: InternedScheduleSet) -> Self {
        if let Some(old_set) = self.system_set {
            let caller = DebugLocation::caller();
            log::error!(
                "Duplicated system set configuration, old: `{old_set:?}`, \
                new: `{set:?}`, the old is overwritten. {caller}"
            );
        }
        self.system_set = Some(set);
        self
    }

    fn merge(mut self, mut other: Self) -> Self {
        self.idents.append(&mut other.idents);
        self.systems.extend(other.systems);
        self.deferred.extend(other.deferred);
        self.dependency.extend(other.dependency);
        self.condition.extend(other.condition);
        self
    }
}

/// Converts values into a [`SystemConfig`] that can be inserted into a schedule.
///
/// This trait is implemented for:
/// - action systems (`IntoSystem<(), (), _>`),
/// - condition systems (`IntoSystem<(), bool, _>`),
/// - tuples of other `IntoSystemConfig` values,
/// - and [`SystemConfig`] itself.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not describe a valid system configuration",
    label = "invalid system configuration"
)]
pub trait IntoSystemConfig<Marker>: Sized {
    fn into_config(self) -> SystemConfig;

    /// Adds `system` so that existing systems run before it.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn before<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().before_impl(s, false)
    }

    /// Adds `system` so that it runs before existing systems.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn after<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().after_impl(s, false)
    }

    /// Adds chain dependencies between adjacent configured systems.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn chain(self) -> SystemConfig {
        self.into_config().chain_impl(false)
    }

    /// Adds a condition system that gates all currently configured systems.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn run_if<M>(self, c: impl IntoSystem<(), bool, M>) -> SystemConfig {
        self.into_config().run_if_impl(c)
    }

    /// Adds an action system as a run-condition producer for configured systems.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn run_if_run<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().run_if_run_impl(s)
    }

    /// Like [`IntoSystemConfig::before`], but skips deferred synchronization edges.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn before_ignore_deferred<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().before_impl(s, true)
    }

    /// Like [`IntoSystemConfig::after`], but skips deferred synchronization edges.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn after_ignore_deferred<M>(self, s: impl IntoSystem<(), (), M>) -> SystemConfig {
        self.into_config().after_impl(s, true)
    }

    /// Like [`IntoSystemConfig::chain`], but skips deferred synchronization edges.
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn chain_ignore_deferred(self) -> SystemConfig {
        self.into_config().chain_impl(true)
    }

    /// Add all internal systems to the target SystemSet.
    ///
    /// By default, the system will be added to the [`AnonymousSystemSet`].
    ///
    /// This function prohibits repeated calls.
    /// We strongly recommend that any system belongs to only one system set.
    ///
    /// [`AnonymousSystemSet`]: super::AnonymousSystemSet
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn in_set(self, set: impl SystemSet) -> SystemConfig {
        self.into_config().in_set_impl(set.intern())
    }
}

impl IntoSystemConfig<()> for SystemConfig {
    #[inline(always)]
    fn into_config(self) -> Self {
        self
    }
}

impl<F, M> IntoSystemConfig<(fn(), M)> for F
where
    F: IntoSystem<(), (), M>,
{
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn into_config(self) -> SystemConfig {
        SystemConfig::new().before_impl(self, false)
    }
}

impl<F, M> IntoSystemConfig<(fn() -> bool, M)> for F
where
    F: IntoSystem<(), bool, M>,
{
    #[cfg_attr(any(debug_assertions, feature = "debug"), track_caller)]
    fn into_config(self) -> SystemConfig {
        SystemConfig::new().run_if_impl(self)
    }
}

impl IntoSystemConfig<()> for () {
    #[inline]
    fn into_config(self) -> SystemConfig {
        SystemConfig::new()
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

#[cfg(test)]
mod tests {
    use super::IntoSystemConfig;
    use crate::system::{IntoSystem, SystemId};

    fn sys_a() {}
    fn sys_b() {}
    fn sys_c() {}
    fn cond_true() -> bool {
        true
    }

    #[test]
    fn before_adds_existing_to_new_dependency() {
        let a_id: SystemId = sys_a.system_id();
        let b_id: SystemId = sys_b.system_id();

        let config = sys_a.before(sys_b).into_config();
        assert!(config.dependency.contains(&(a_id, b_id)));
    }

    #[test]
    fn after_adds_new_to_existing_dependency() {
        let a_id: SystemId = sys_a.system_id();
        let b_id: SystemId = sys_b.system_id();

        let config = sys_a.after(sys_b).into_config();
        assert!(config.dependency.contains(&(b_id, a_id)));
    }

    #[test]
    fn chain_adds_linear_dependencies() {
        let a_id: SystemId = sys_a.system_id();
        let b_id: SystemId = sys_b.system_id();
        let c_id: SystemId = sys_c.system_id();

        let config = (sys_a, sys_b, sys_c).chain().into_config();
        assert!(config.dependency.contains(&(a_id, b_id)));
        assert!(config.dependency.contains(&(b_id, c_id)));
    }

    #[test]
    fn run_if_adds_condition_edge() {
        let cond_id: SystemId = cond_true.system_id();
        let a_id: SystemId = sys_a.system_id();

        let config = sys_a.run_if(cond_true).into_config();
        assert!(config.condition.contains(&(cond_id, a_id)));
    }
}
