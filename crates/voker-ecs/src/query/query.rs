#![expect(clippy::module_inception, reason = "For better structure.")]

use core::fmt::Debug;

use super::{QueryData, QueryFilter, QueryState, ReadOnlyQueryData};
use crate::system::{AccessTable, ReadOnlySystemParam};
use crate::system::{SystemParam, SystemParamError};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// Query

/// A parameter for querying components and entities from the ECS world.
///
/// `Query` contains two type parameters: [`QueryData`] (what to fetch) and
/// [`QueryFilter`] (filtering conditions, defaults to no filtering).
///
/// # Examples
///
/// ```ignore
/// // Basic component query
/// fn system1(query: Query<&Foo>) {
///     for foo in query {
///         /* ... */
///     }
/// }
///
/// // Query with tuple and filter
/// fn system2(query: Query<(Entity, &Foo), With<Bar>>) {
///     for (entity, foo) in query {
///         /* ... */
///     }
/// }
///
/// // Complex filter composition
/// fn system3(query: Query<(Entity, &Foo), And<(With<Bar>, Without<Baz>, Changed<Foo>)>>) {
///     for (entity, foo) in query {
///         /* ... */
///     }
/// }
/// ```
///
/// # Query Data Types
///
/// The following types can be used as query data (implement [`QueryData`]):
///
/// - **Entity handles**: `Entity`, `EntityRef`, `EntityMut`
/// - **Component references**: `&T`, `&mut T`, `Ref<T>`, `Mut<T>` where `T` is a component type
/// - **Optional components**: `Option<&T>`, `Option<&mut T>`, `Option<Ref<T>>`, `Option<Mut<T>>`
///
/// # Query Filter Types
///
/// The following filters are available (implement [`QueryFilter`]):
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
/// For custom implementations, refer to the [`QueryData`] and [`QueryFilter`] traits.
///
/// # Implementation & Optimization
///
/// Query execution follows a two-phase filtering strategy:
///
/// 1. **Archetype-based filtering**: Quickly eliminates entire archetypes that cannot
///    possibly match the query criteria.
/// 2. **Entity-based filtering**: Performs fine-grained filtering on individual entities
///    during iteration.
///
/// ## Performance Characteristics
///
/// The query system provides predictable performance with the following complexities:
///
/// | Phase | Complexity | Description |
/// |-------|------------|-------------|
/// | **Archetype filtering** | `O(NA × NC × log NC)` | Where `NA` is the number of *incrementally updated* archetypes and `NC` is the number of components involved in filters. This cost is amortized through caching. |
/// | **Entity iteration** | `O(NE)` | Where `NE` is the number of entities in matching archetypes. Iteration overhead is minimal and linear in result count. |
///
/// ## Optimizations
///
/// 1. **Archetype caching**: [`QueryState`] caches the results of archetype-based filtering,
///    eliminating repeated archetype traversal. The cache is maintained incrementally
///    as archetypes are created or modified.
///
/// 2. **Thin handle**: [`Query`] itself is a lightweight handle (essentially a pointer to
///    [`QueryState`]) that doesn't perform entity-level filtering. The actual filtering
///    occurs when creating and iterating a [`QueryIter`].
///
/// 3. **Filter elimination**: Simple filters (like `With`/`Without`) can be evaluated
///    entirely at the archetype level. If no complex filters (e.g., `Changed`/`Added`)
///    are present, the entity-level filtering can be completely optimized away at compile
///    time - all entities in matching archetypes are valid results.
///
/// 4. **Cache-efficient iteration**: For queries that don't involve sparse components,
///    iteration is organized by table rather than archetype. This maximizes cache locality
///    as entities within the same table are stored contiguously in memory.
///
/// [`Archetype`]: crate::archetype::Archetype
/// [`QueryIter`]: crate::query::QueryIter
/// [`QueryState`]: crate::query::QueryState
pub struct Query<'world, 'state, D: QueryData, F: QueryFilter = ()> {
    pub(super) world: UnsafeWorld<'world>,
    pub(super) state: &'state QueryState<D, F>,
    pub(super) last_run: Tick,
    pub(super) this_run: Tick,
}

// -----------------------------------------------------------------------------
// Query -> SystemParam

unsafe impl<D: QueryData + 'static, F: QueryFilter + 'static> SystemParam for Query<'_, '_, D, F> {
    type State = QueryState<D, F>;
    type Item<'world, 'state> = Query<'world, 'state, D, F>;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        QueryState::new(world)
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        state.mark_assess(table)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        state.update(unsafe { world.read_only() });
        Ok(Query {
            world,
            state,
            last_run,
            this_run,
        })
    }
}

impl<D: QueryData, F: QueryFilter> Debug for Query<'_, '_, D, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Query")
            .field("state", &self.state)
            .field("last_run", &self.last_run)
            .field("this_run", &self.this_run)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// Query implementation

impl<'w, 's, D: QueryData, F: QueryFilter> Query<'w, 's, D, F> {
    /// Returns a reborrowed query with a shorter world lifetime.
    ///
    /// This is mainly useful when the query contains mutable borrows and you
    /// need to pass a temporary query handle to helper functions while keeping
    /// the original query available afterward.
    ///
    /// If the query is read-only, [`Query`] itself implements [`Copy`], so
    /// reborrowing is usually unnecessary.
    pub fn reborrow(&self) -> Query<'_, 's, D, F> {
        Query {
            world: self.world,
            state: self.state,
            last_run: self.last_run,
            this_run: self.this_run,
        }
    }
}

// -----------------------------------------------------------------------------
// ReadOnlyQuery

unsafe impl<D: ReadOnlyQueryData + 'static, F: QueryFilter + 'static> ReadOnlySystemParam
    for Query<'_, '_, D, F>
{
}

impl<D: ReadOnlyQueryData, F: QueryFilter> Copy for Query<'_, '_, D, F> {}

impl<D: ReadOnlyQueryData, F: QueryFilter> Clone for Query<'_, '_, D, F> {
    fn clone(&self) -> Self {
        *self
    }
}
