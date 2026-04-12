mod comp;
mod entity;
mod tuples;

// -----------------------------------------------------------------------------
// QueryData

use alloc::vec::Vec;

use crate::archetype::Archetype;
use crate::entity::Entity;
use crate::storage::{Table, TableRow};
use crate::system::{AccessParam, FilterParamBuilder};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

/// Core trait for types that can be fetched from entities in a query.
///
/// This trait defines how a query accesses data from entities. It is implemented
/// for component references, tuples of components, and other data sources.
///
/// # Derive macro usage
///
/// Prefer [`#[derive(QueryData)]`](crate::derive::QueryData) for custom query-data structs.
///
/// Rules enforced by the derive:
/// - The struct may have no lifetime params, or exactly one lifetime named `'w`.
/// - For mutable fields, use [`crate::borrow::Mut`] instead of `&'w mut T`.
/// - Add `#[query_data(readonly)]` when the derived type is read-only and
///   should also implement [`ReadOnlyQueryData`].
///
/// Example:
///
/// ```no_run
/// use voker_ecs::borrow::Mut;
/// use voker_ecs::derive::{Component, QueryData};
///
/// #[derive(Component, Clone)]
/// struct Position { x: f32, y: f32 }
///
/// #[derive(Component, Clone)]
/// struct Velocity { x: f32, y: f32 }
///
/// #[derive(QueryData)]
/// #[query_data(readonly)]
/// struct ReadVelocity<'w> {
///     velocity: &'w Velocity,
/// }
///
/// #[derive(QueryData)]
/// struct MoveData<'w> {
///     position: Mut<'w, Position>,
///     velocity: &'w Velocity,
/// }
/// ```
///
/// # Available Params
///
/// The following query data forms are supported:
///
/// - **Entity handles**: `Entity`, `EntityRef`, `EntityMut`
/// - **Component references**: `&T`, `&mut T`, `Ref<T>`, `Mut<T>` where `T` is a component type
/// - **Optional components**: `Option<&T>`, `Option<&mut T>`, `Option<Ref<T>>`, `Option<Mut<T>>`
///
/// For mutable component forms, `Item<'world>` is [`crate::borrow::Mut`] (or
/// `Option<Mut<_>>`) rather than raw `&mut T`, so change ticks remain attached.
///
/// Tuples composed from these forms are also valid, for example `(&Foo, &mut Bar)`.
///
/// # Aliasing rules
///
/// `QueryData` must obey Rust aliasing rules. For example, `(&Foo, &mut Foo)` is
/// invalid and will panic at runtime.
///
/// Also note the difference between entity-only and entity-wide access:
/// - `Entity` carries only an entity ID and does not access components.
/// - `EntityRef` represents shared access to all components on that entity.
/// - `EntityMut` represents exclusive access to all components on that entity.
///
/// Therefore, `(EntityRef, &Foo)` is valid, while `(EntityRef, &mut Foo)` and
/// `(EntityMut, &Foo)` are invalid and will panic at runtime.
///
/// # Safety
///
/// Implementing this trait requires careful attention to memory safety and
/// component access patterns. See trait methods for specific safety requirements.
///
/// The `QueryData::Item` should not need `Drop`.
///
/// Implementations should treat `build_filter` and `build_access` as separate
/// concerns:
/// - filter describes which storages/entities are candidates,
/// - access describes what data might be read or written for scheduler conflict checks.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a query data",
    label = "invalid query data",
    note = "Consider annotating `{Self}` with `#[derive(QueryData)]`."
)]
pub unsafe trait QueryData {
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

    /// The type returned when fetching data for a single entity.
    ///
    /// The data should be lightweight and not contain any content
    /// that needs to be dropped.
    type Item<'world>;

    /// Indicates whether all components accessed by this filter use dense storage.
    ///
    /// - If `true`, the query can optimize by assuming components are stored in tables.
    /// - If `false`, the filter may access sparse components requiring map lookups.
    const COMPONENTS_ARE_DENSE: bool;

    /// Builds the static state for this query data.
    ///
    /// This is called once when the query is first created. The state is
    /// shared across all query executions and contains metadata needed for
    /// future cache building and fetching.
    fn build_state(world: &mut World) -> Self::State;

    /// Try get the static state from given world without mutable reference.
    fn fetch_state(world: &World) -> Option<Self::State>;

    /// Builds a per-execution cache for this query data.
    ///
    /// This is called at the beginning of each query execution to prepare
    /// world-specific data needed for fetching. The cache may contain direct
    /// pointers to component arrays or other performance-critical data.
    ///
    /// # Safety
    /// - The returned cache must remain valid for the duration of the query
    /// - World access must follow the provided tick parameters
    /// - Pointers stored in cache must remain valid while cache is alive
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
    /// Therefore, implementations of [`QueryData::build_filter`] usually add
    /// requirements to every existing branch, instead of creating new branches.
    ///
    /// [`QueryFilter::build_filter`]: crate::query::QueryFilter::build_filter
    fn build_filter(state: &Self::State, out: &mut Vec<FilterParamBuilder>);

    /// Builds the set of data this query may access.
    ///
    /// Unlike [`QueryData::build_filter`], which describes archetype matching,
    /// this method describes potential component/resource accesses for system
    /// safety checks (mutual exclusion and aliasing validation).
    ///
    /// For example, `Query<(&mut Foo, &Foo)>` is an invalid access target,
    /// and this function should return `false`.
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
    /// [`QueryFilter::build_access`]: crate::query::QueryFilter::build_access
    fn build_access(state: &Self::State, out: &mut AccessParam) -> bool;

    /// Prepares the cache for a specific archetype.
    ///
    /// Called when the query begins processing a new archetype. The implementation
    /// can pre-compute archetype-specific information to speed up later fetching.
    ///
    /// # Safety
    /// - The archetype must remain valid for the duration of the query
    /// - Cache updates must not invalidate existing data
    /// - Must correctly handle archetype component layout
    unsafe fn set_for_arche<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        arche: &'w Archetype,
        table: &'w Table,
    );

    /// Prepares the cache for a specific table.
    ///
    /// Called when the query begins processing a new table. The implementation
    /// can pre-compute table-specific information to speed up later fetching.
    ///
    /// # Safety
    /// - The table must remain valid for the duration of the query
    /// - Cache updates must not invalidate existing data
    /// - Must correctly handle table column layout
    unsafe fn set_for_table<'w>(state: &Self::State, cache: &mut Self::Cache<'w>, table: &'w Table);

    /// Fetches data for a single entity.
    ///
    /// This is called for each entity that passes all filter conditions.
    /// Returns `Some(item)` if the entity has the requested data, or `None`
    /// if the data is not available (for optional fetches).
    ///
    /// # Safety
    /// - The entity must exist and be valid
    /// - The table row must be valid for the current table
    /// - Cache must be properly set for the current archetype/table
    /// - Returned references must follow Rust's borrowing rules
    unsafe fn fetch<'w>(
        state: &Self::State,
        cache: &mut Self::Cache<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Option<Self::Item<'w>>;
}

/// A trait that ensures the query is read-only.
///
/// Then the `Query` implemented `Copy`,
/// and the `QueryState` can be used with immutable world reference.
///
/// # Safety
/// The implementer ensures it's read only.
pub unsafe trait ReadOnlyQueryData: QueryData {}
