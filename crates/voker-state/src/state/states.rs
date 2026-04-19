use core::fmt::Debug;
use core::hash::Hash;

// -----------------------------------------------------------------------------
// States

#[diagnostic::on_unimplemented(
    message = "`{Self}` can not be used as a state",
    label = "invalid state",
    note = "consider annotating `{Self}` with `#[derive(States)]`"
)]
pub trait States: 'static + Send + Sync + Clone + Eq + Hash + Debug {
    /// Dependency depth used to order derived/sub-state transitions.
    ///
    /// Base/manual states default to `1`. Derived/sub-states build on top of
    /// source depths.
    const DEPENDENCY_DEPTH: usize = 1;
}
