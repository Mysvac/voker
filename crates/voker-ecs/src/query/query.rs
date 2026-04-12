#![expect(clippy::module_inception, reason = "For better structure.")]

use core::fmt::Debug;
use core::mem::MaybeUninit;

use super::error::{QueryEntityError, QuerySingleError};
use super::{QueryData, QueryFilter, QueryIter, QueryState, ReadOnlyQueryData};
use crate::entity::{Entity, EntityLocation, StorageId};
use crate::query::Single;
use crate::system::{AccessTable, SystemParam, SystemParamError};
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
/// Mutable forms (`&mut T`, `Option<&mut T>`) yield [`crate::borrow::Mut`] at
/// iteration/fetch time, so change-tracking metadata is preserved.
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
        QueryState::build(world)
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
        unsafe { Ok(Query::new(world, state, last_run, this_run)) }
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
// IntoIterator

impl<'w, 's, D: QueryData, F: QueryFilter> IntoIterator for Query<'w, 's, D, F> {
    type Item = D::Item<'w>;
    type IntoIter = QueryIter<'w, 's, D, F>;

    fn into_iter(self) -> Self::IntoIter {
        unsafe { QueryIter::new(self.world, self.state, self.last_run, self.this_run) }
    }
}

impl<'a, 'w: 'a, 's, D: ReadOnlyQueryData, F: QueryFilter> IntoIterator
    for &'a Query<'w, 's, D, F>
{
    type Item = D::Item<'a>;
    type IntoIter = QueryIter<'a, 's, D, F>;

    fn into_iter(self) -> Self::IntoIter {
        unsafe { QueryIter::new(self.world, self.state, self.last_run, self.this_run) }
    }
}

impl<'a, 'w: 'a, 's, D: QueryData, F: QueryFilter> IntoIterator for &'a mut Query<'w, 's, D, F> {
    type Item = D::Item<'a>;
    type IntoIter = QueryIter<'a, 's, D, F>;

    fn into_iter(self) -> Self::IntoIter {
        unsafe { QueryIter::new(self.world, self.state, self.last_run, self.this_run) }
    }
}

// -----------------------------------------------------------------------------
// Query implementation

impl<'w, 's, D: QueryData, F: QueryFilter> Query<'w, 's, D, F> {
    #[inline]
    pub unsafe fn new(
        world: UnsafeWorld<'w>,
        state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        Query {
            world,
            state,
            last_run,
            this_run,
        }
    }

    /// Returns a reborrowed query with a shorter world lifetime.
    ///
    /// This is mainly useful when the query contains mutable borrows and you
    /// need to pass a temporary query handle to helper functions while keeping
    /// the original query available afterward.
    ///
    /// If the query is read-only, [`Query`] itself implements [`Copy`], so
    /// reborrowing is usually unnecessary.
    pub fn reborrow(&mut self) -> Query<'_, 's, D, F> {
        Query {
            world: self.world,
            state: self.state,
            last_run: self.last_run,
            this_run: self.this_run,
        }
    }

    /// Returns a mutable iterator over query results.
    pub fn iter_mut(&mut self) -> QueryIter<'_, 's, D, F> {
        unsafe { QueryIter::new(self.world, self.state, self.last_run, self.this_run) }
    }

    /// Returns a read-only iterator over query results.
    pub fn iter(&self) -> QueryIter<'w, 's, D, F>
    where
        D: ReadOnlyQueryData,
    {
        unsafe { QueryIter::new(self.world, self.state, self.last_run, self.this_run) }
    }

    pub fn single_mut(&mut self) -> Result<Single<'_, D, F>, QuerySingleError> {
        unsafe { Single::new(self.world, self.state, self.last_run, self.this_run) }
    }

    pub fn single(&self) -> Result<Single<'w, D, F>, QuerySingleError>
    where
        D: ReadOnlyQueryData,
    {
        unsafe { Single::new(self.world, self.state, self.last_run, self.this_run) }
    }

    /// Fetches one entity from this query with mutable query access.
    ///
    /// Returns [`QueryEntityError::NoSuchEntity`] if the entity is stale or
    /// despawned, and [`QueryEntityError::QueryMismatch`] if it does not satisfy
    /// this query.
    pub fn get_mut(&mut self, entity: Entity) -> Result<D::Item<'_>, QueryEntityError> {
        unsafe { self.get_impl(entity) }
    }

    /// Fetches one entity from this query with read-only query access.
    pub fn get(&self, entity: Entity) -> Result<D::Item<'w>, QueryEntityError>
    where
        D: ReadOnlyQueryData,
    {
        unsafe { self.get_impl(entity) }
    }

    /// Fetches multiple entities from this query with mutable query access.
    ///
    /// Returns [`QueryEntityError::DuplicateEntity`] if any input entity is
    /// repeated.
    pub fn get_many_mut<const N: usize>(
        &mut self,
        entities: [Entity; N],
    ) -> Result<[D::Item<'_>; N], QueryEntityError> {
        unsafe { self.get_many_mut_impl(entities) }
    }

    /// Fetches multiple entities from this query with read-only query access.
    pub fn get_many<const N: usize>(
        &self,
        entities: [Entity; N],
    ) -> Result<[D::Item<'w>; N], QueryEntityError>
    where
        D: ReadOnlyQueryData,
    {
        unsafe { self.get_many_impl(entities) }
    }

    /// Returns `true` if this query currently has no matches.
    pub fn is_empty(&self) -> bool {
        unsafe {
            QueryIter::new(self.world, self.state, self.last_run, self.this_run)
                .next()
                .is_none()
        }
    }

    /// Returns `true` if `entity` currently satisfies this query.
    pub fn contains(&self, entity: Entity) -> bool {
        self.contains_impl(entity)
    }
}

// -----------------------------------------------------------------------------
// ReadOnlyQuery

impl<D: ReadOnlyQueryData, F: QueryFilter> Copy for Query<'_, '_, D, F> {}

impl<D: ReadOnlyQueryData, F: QueryFilter> Clone for Query<'_, '_, D, F> {
    fn clone(&self) -> Self {
        *self
    }
}

// -----------------------------------------------------------------------------
// Helper

impl<'w, 's, D: QueryData, F: QueryFilter> Query<'w, 's, D, F> {
    #[inline]
    fn get_storage_id(location: EntityLocation) -> StorageId {
        if QueryState::<D, F>::IS_DENSE {
            StorageId {
                table_id: location.table_id,
            }
        } else {
            StorageId {
                arche_id: location.arche_id,
            }
        }
    }

    #[inline]
    fn contains_storage(&self, storage: StorageId) -> bool {
        use crate::utils::contains_storage_id;
        let storages = self.state.storages.as_slice();
        if storages.len() <= 200 {
            contains_storage_id(storage, storages)
        } else {
            storages.binary_search(&storage).is_ok()
        }
    }

    #[inline]
    fn locate_entity(&self, entity: Entity) -> Result<EntityLocation, QueryEntityError> {
        let entities = unsafe { &self.world.read_only().entities };
        entities
            .locate(entity)
            .map_err(|_| QueryEntityError::NoSuchEntity(entity))
    }

    #[inline]
    unsafe fn update_filter_cache(&self, f_cache: &mut F::Cache<'w>, location: EntityLocation) {
        if QueryState::<D, F>::IS_DENSE {
            let read_world = unsafe { self.world.read_only() };
            let table = unsafe { read_world.storages.tables.get_unchecked(location.table_id) };
            unsafe { F::set_for_table(&self.state.f_state, f_cache, table) };
        } else {
            let read_world = unsafe { self.world.read_only() };
            let arche = unsafe { read_world.archetypes.get_unchecked(location.arche_id) };
            let table = unsafe { read_world.storages.tables.get_unchecked(location.table_id) };
            unsafe { F::set_for_arche(&self.state.f_state, f_cache, arche, table) };
        }
    }

    #[inline]
    unsafe fn update_data_cache(&self, d_cache: &mut D::Cache<'w>, location: EntityLocation) {
        if QueryState::<D, F>::IS_DENSE {
            let read_world = unsafe { self.world.read_only() };
            let table = unsafe { read_world.storages.tables.get_unchecked(location.table_id) };
            unsafe { D::set_for_table(&self.state.d_state, d_cache, table) };
        } else {
            let read_world = unsafe { self.world.read_only() };
            let arche = unsafe { read_world.archetypes.get_unchecked(location.arche_id) };
            let table = unsafe { read_world.storages.tables.get_unchecked(location.table_id) };
            unsafe { D::set_for_arche(&self.state.d_state, d_cache, arche, table) };
        }
    }

    fn contains_impl(&self, entity: Entity) -> bool {
        let world = self.world;
        let this_run = self.this_run;
        let last_run = self.last_run;

        let Ok(location) = self.locate_entity(entity) else {
            return false;
        };

        let storage = Self::get_storage_id(location);
        if !self.contains_storage(storage) {
            return false;
        }

        if F::ENABLE_ENTITY_FILTER {
            unsafe {
                let mut f_cache = F::build_cache(&self.state.f_state, world, last_run, this_run);
                self.update_filter_cache(&mut f_cache, location);
                if !F::filter(
                    &self.state.f_state,
                    &mut f_cache,
                    entity,
                    location.table_row,
                ) {
                    return false;
                }
            }
        }

        unsafe {
            let mut d_cache = D::build_cache(&self.state.d_state, world, last_run, this_run);
            self.update_data_cache(&mut d_cache, location);
            D::fetch(
                &self.state.d_state,
                &mut d_cache,
                entity,
                location.table_row,
            )
            .is_some()
        }
    }

    pub(super) unsafe fn get_impl(&self, entity: Entity) -> Result<D::Item<'w>, QueryEntityError> {
        let world = self.world;
        let this_run = self.this_run;
        let last_run = self.last_run;

        let location = self.locate_entity(entity)?;

        let storage = Self::get_storage_id(location);
        if !self.contains_storage(storage) {
            return Err(QueryEntityError::QueryMismatch(entity));
        }

        if F::ENABLE_ENTITY_FILTER {
            unsafe {
                let mut f_cache = F::build_cache(&self.state.f_state, world, last_run, this_run);
                self.update_filter_cache(&mut f_cache, location);
                if !F::filter(
                    &self.state.f_state,
                    &mut f_cache,
                    entity,
                    location.table_row,
                ) {
                    return Err(QueryEntityError::QueryMismatch(entity));
                }
            }
        }

        unsafe {
            let mut d_cache = D::build_cache(&self.state.d_state, world, last_run, this_run);
            self.update_data_cache(&mut d_cache, location);
            D::fetch(
                &self.state.d_state,
                &mut d_cache,
                entity,
                location.table_row,
            )
            .ok_or(QueryEntityError::QueryMismatch(entity))
        }
    }

    pub(super) unsafe fn get_many_mut_impl<const N: usize>(
        &self,
        entities: [Entity; N],
    ) -> Result<[D::Item<'w>; N], QueryEntityError> {
        use crate::utils::contains_entity;

        for i in 0..N {
            if contains_entity(entities[i], &entities[0..i]) {
                return Err(QueryEntityError::DuplicateEntity(entities[i]));
            }
        }

        unsafe { self.get_many_impl(entities) }
    }

    pub(super) unsafe fn get_many_impl<const N: usize>(
        &self,
        entities: [Entity; N],
    ) -> Result<[D::Item<'w>; N], QueryEntityError> {
        let world = self.world;
        let this_run = self.this_run;
        let last_run = self.last_run;

        let mut values = [const { MaybeUninit::<D::Item<'w>>::uninit() }; N];

        let mut f_cache = unsafe { F::build_cache(&self.state.f_state, world, last_run, this_run) };
        let mut d_cache = unsafe { D::build_cache(&self.state.d_state, world, last_run, this_run) };

        for (value, entity) in core::iter::zip(&mut values, entities) {
            // SAFETY: We guarantee that QueryData::ITEM does not need Drop.
            let item = unsafe { self.get_with_cache_impl(entity, &mut f_cache, &mut d_cache)? };
            *value = MaybeUninit::new(item);
        }

        Ok(values.map(|x| unsafe { x.assume_init() }))
    }

    unsafe fn get_with_cache_impl(
        &self,
        entity: Entity,
        f_cache: &mut F::Cache<'w>,
        d_cache: &mut D::Cache<'w>,
    ) -> Result<D::Item<'w>, QueryEntityError> {
        let location = self.locate_entity(entity)?;

        let storage = Self::get_storage_id(location);
        if !self.contains_storage(storage) {
            return Err(QueryEntityError::QueryMismatch(entity));
        }

        if F::ENABLE_ENTITY_FILTER {
            unsafe {
                self.update_filter_cache(f_cache, location);
                if !F::filter(&self.state.f_state, f_cache, entity, location.table_row) {
                    return Err(QueryEntityError::QueryMismatch(entity));
                }
            }
        }

        unsafe {
            self.update_data_cache(d_cache, location);
            D::fetch(&self.state.d_state, d_cache, entity, location.table_row)
                .ok_or(QueryEntityError::QueryMismatch(entity))
        }
    }
}
