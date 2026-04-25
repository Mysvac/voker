//! [`QueryFilter`] implementations — the predicate half of a query.
//!
//! Filters refine which entities a query visits without changing what data
//! is fetched: `With<T>`, `Without<T>`, `Added<T>`, `Changed<T>`, and
//! logical combinators `And`/`Or`.

mod added;
mod and;
mod changed;
mod or;
mod with;
mod without;

pub use added::Added;
pub use and::And;
pub use changed::Changed;
pub use or::Or;
pub use with::With;
pub use without::Without;

// -----------------------------------------------------------------------------
// QueryFilter

use alloc::vec::Vec;

use crate::archetype::Archetype;
use crate::entity::Entity;
use crate::storage::{Table, TableRow};
use crate::system::{AccessParam, AccessTable, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

/// Core trait for types that can filter entities in a query.
///
/// Filters can participate in two levels:
/// - storage-level prefiltering (`build_filter`) for fast coarse pruning,
/// - entity-level checks (`filter`) for per-row decisions.
///
/// Implementations should push as much work as possible into storage-level
/// filtering to reduce per-entity overhead.
///
/// The following filters are available:
///
/// | Filter | Description |
/// |--------|-------------|
/// | `And<(F1, F2, ...)>` | Logical AND - all inner filters must be satisfied |
/// | `Or<(F1, F2, ...)>` | Logical OR - at least one inner filter must be satisfied |
/// | `With<C>` | Requires the entity to have component `C` |
/// | `With<(C1, C2, ...)>` | Requires the entity to have all specified components |
/// | `Without<C>` | Requires the entity to NOT have component `C` |
/// | `Without<(C1, C2, ...)>` | Requires the entity to have none of the specified components |
/// | `Changed<C>` | Component `C` must have been modified in the interval `(last_run, this_run]` |
/// | `Added<C>` | Component `C` must have been added in the interval `(last_run, this_run]` |
///
/// # Type Parameters
///
/// - [`QueryFilter::State`] - Static data shared across all query instances
/// - [`QueryFilter::Cache`] - Per-execution cached data for a specific world state
///
/// # Safety
///
/// Implementing this trait requires careful attention to memory safety and
/// component access patterns. See trait methods for specific safety requirements.
pub unsafe trait QueryFilter {
    /// Static data shared across all query instances.
    ///
    /// This is typically built once during query construction and contains
    /// information like component IDs that don't change over the query's lifetime.
    type State: Clone + Send + Sync + 'static;

    /// Per-query cached data for a specific world state.
    ///
    /// This cache is rebuilt each time the query is executed and may contain
    /// world-specific data like component pointers or pre-computed lookup tables.
    type Cache<'world>;

    /// Indicates whether all components accessed by this filter use dense storage.
    ///
    /// - If `true`, the query can optimize by assuming components are stored in tables.
    /// - If `false`, the filter may access sparse components requiring map lookups.
    const COMPONENTS_ARE_DENSE: bool;

    /// Indicates whether this filter performs per-entity filtering.
    ///
    /// If `false`, the filter can be fully evaluated at the archetype/table level,
    /// allowing for optimizations like skipping the per-entity filter loop.
    ///
    /// Example: `With<T>` usually does not require per-entity checks, while
    /// tick-based predicates like `Changed<T>` generally do.
    const ENABLE_ENTITY_FILTER: bool;

    /// Builds the static state for this filter.
    ///
    /// This is called once when the query is first created. The state is
    /// shared across all query executions.
    fn build_state(world: &mut World) -> Self::State;

    /// Try get the static state from given world without mutable reference.
    fn try_build_state(world: &World) -> Option<Self::State>;

    /// Builds a per-execution cache for this filter.
    ///
    /// This is called at the beginning of each query execution to prepare
    /// world-specific data needed for filtering.
    ///
    /// # Safety
    /// - The returned cache must remain valid for the duration of the query
    /// - World access must follow the provided tick parameters
    unsafe fn build_cache<'w>(
        state: &Self::State,
        world: UnsafeWorld<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Cache<'w>;

    /// Builds archetype-level filter parameters.
    ///
    /// This contributes constraints used during archetype filtering.
    /// The `out` vector is in disjunctive-normal-form style: each item is one
    /// `Or` branch, and the query matches if any branch is satisfied.
    ///
    /// # Note
    ///
    /// The caller must ensure that [`QueryFilter::build_filter`] is called **before**
    /// [`QueryData::build_filter`].
    ///
    /// For [`QueryFilter::build_filter`] implementations, new branches typically
    /// need to be added. By default, the input `out` is an empty vector, meaning
    /// no archetype would satisfy the filter conditions.
    ///
    /// [`QueryData::build_filter`]: crate::query::QueryData::build_filter
    fn build_filter(state: &Self::State, out: &mut Vec<FilterParamBuilder>);

    /// Builds the set of data this query may access.
    ///
    /// Unlike [`QueryFilter::build_filter`], which describes archetype matching,
    /// this method describes potential component/resource accesses for system
    /// safety checks (mutual exclusion and aliasing validation).
    ///
    /// For example, `Query<(..), Changed<Foo>>` need to access the change tick of `Foo`.
    ///
    /// Because `QueryFilter` is read only and evaluated during iterator filtering,
    /// it's always valid.
    ///
    /// # Note
    ///
    /// The caller must ensure that [`QueryFilter::build_access`] is called **after**
    /// [`QueryData::build_access`].
    ///
    /// `QueryFilter` target accesses are evaluated during iterator filtering and
    /// do not conflict with `QueryData` target registration, so `QueryData`
    /// should register first.
    ///
    /// [`QueryData::build_access`]: crate::query::QueryData::build_access
    fn build_access(state: &Self::State, out: &mut AccessParam);

    /// Explicitly edit access table.
    ///
    /// This is usually only used for queries that require access to resources.
    ///
    /// ## TODO:
    /// It doesn't seem good to directly open this interface, a better approach
    /// is to provide an interface that only allows access to editing resources.
    fn edit_access_table(state: &Self::State, table: &mut AccessTable) -> bool;

    /// Updates the cache for a specific archetype.
    ///
    /// Called when the query begins processing a new archetype. The filter
    /// can pre-compute archetype-level information to speed up later filtering.
    ///
    /// # Safety
    /// - The archetype must remain valid for the duration of the query
    /// - Cache updates must not invalidate existing data
    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        arche: &'w Archetype,
        table: &'w Table,
    );

    /// Updates the cache for a specific table.
    ///
    /// Called when the query begins processing a new table. The filter
    /// can pre-compute table-level information to speed up later filtering.
    ///
    /// # Safety
    /// - The table must remain valid for the duration of the query
    /// - Cache updates must not invalidate existing data
    unsafe fn set_for_table<'w>(state: &Self::State, cache: &mut Self::Cache<'w>, table: &'w Table);

    /// Performs per-entity filtering.
    ///
    /// This is called for each entity that passes archetype/table-level checks.
    /// Returns `true` if the entity should be included in query results.
    ///
    /// # Safety
    /// - The entity must exist and be valid
    /// - The table row must be valid for the current table
    /// - Cache data must be properly set for the current archetype/table
    unsafe fn filter<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool;
}

// -----------------------------------------------------------------------------
// empty

unsafe impl QueryFilter for () {
    type State = ();
    type Cache<'world> = ();

    const COMPONENTS_ARE_DENSE: bool = true;
    const ENABLE_ENTITY_FILTER: bool = false;

    fn build_state(_world: &mut World) -> Self::State {}

    fn try_build_state(_world: &World) -> Option<Self::State> {
        Some(())
    }

    unsafe fn build_cache<'w>(
        _state: &Self::State,
        _world: UnsafeWorld<'w>,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Self::Cache<'w> {
    }

    fn build_filter(_state: &Self::State, outer: &mut Vec<FilterParamBuilder>) {
        outer.push(FilterParamBuilder::new());
    }

    fn build_access(_state: &Self::State, _out: &mut AccessParam) {}

    fn edit_access_table(_state: &Self::State, _table: &mut AccessTable) -> bool {
        true
    }

    unsafe fn set_for_arche<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _arche: &'w Archetype,
        _table: &'w Table,
    ) {
    }

    unsafe fn set_for_table<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _table: &'w Table,
    ) {
    }

    unsafe fn filter<'w>(
        _state: &Self::State,
        _cache: &mut Self::Cache<'w>,
        _entity: Entity,
        _table_row: TableRow,
    ) -> bool {
        true
    }
}
