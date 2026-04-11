use core::iter::FusedIterator;

use super::{Query, QueryData, QueryFilter, QueryState, ReadOnlyQueryData};
use crate::entity::{Entity, StorageId};
use crate::storage::TableRow;
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

// -----------------------------------------------------------------------------
// QueryIter

/// Iterator over query results.
///
/// This iterator traverses matched storages from [`QueryState`], applies
/// optional entity-level filtering, and fetches query items lazily.
///
/// It can be obtained from:
/// - [`QueryState::iter_mut`]
/// - [`QueryState::iter`] for read-only data
/// - [`Query::iter_mut`]
/// - [`Query::iter`] for read-only data
/// - `Query` by value via [`IntoIterator::into_iter`]
/// - `&mut Query` via [`IntoIterator::into_iter`]
/// - `&Query` via [`IntoIterator::into_iter`] for read-only data
///
/// # Examples
///
/// ```ignore
/// fn system(query: Query<(Entity, &Foo), With<Bar>>) {
///     for (entity, foo) in &query {
///         /* ... */
///     }
/// }
/// ```
pub struct QueryIter<'w, 's, D: QueryData, F: QueryFilter> {
    world: UnsafeWorld<'w>,
    state: &'s QueryState<D, F>,
    d_cache: D::Cache<'w>,
    f_cache: F::Cache<'w>,
    storages: core::slice::Iter<'s, StorageId>,
    entities: &'w [Entity],
    row: usize,
}

// -----------------------------------------------------------------------------
// QueryIter Implementation

const EMPTY_ENTITIES: &[Entity] = &[];

impl<D: QueryData, F: QueryFilter> QueryIter<'_, '_, D, F> {
    /// # Safety
    /// - `world` must be the same world used to build `state`.
    /// - `state` must have access registrations compatible with `D`/`F`.
    /// - `last_run`/`this_run` must belong to the same world tick stream.
    /// - Caller must ensure no aliasing violations are introduced through
    ///   concurrent mutable iteration paths.
    pub(super) unsafe fn new<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> QueryIter<'w, 's, D, F> {
        unsafe {
            QueryIter {
                world,
                state,
                d_cache: D::build_cache(&state.d_state, world, last_run, this_run),
                f_cache: F::build_cache(&state.f_state, world, last_run, this_run),
                storages: state.storages.iter(),
                entities: EMPTY_ENTITIES,
                row: 0,
            }
        }
    }

    /// Advances to the next non-empty storage slice and refreshes caches.
    ///
    /// Returns `None` when no storage remains.
    #[cold]
    #[inline(never)]
    fn update_slice(&mut self) -> Option<()> {
        self.row = 0;
        loop {
            let id = *self.storages.next()?;
            if QueryState::<D, F>::IS_DENSE {
                let table_id = unsafe { id.table_id };
                let storages = unsafe { &self.world.read_only().storages };
                let table = unsafe { storages.tables.get_unchecked(table_id) };
                self.entities = table.entities();
                if !self.entities.is_empty() {
                    unsafe {
                        D::set_for_table(&self.state.d_state, &mut self.d_cache, table);
                        F::set_for_table(&self.state.f_state, &mut self.f_cache, table);
                    }
                    return Some(());
                }
            } else {
                let arche_id = unsafe { id.arche_id };
                let arches = unsafe { &self.world.read_only().archetypes };
                let arche = unsafe { arches.get_unchecked(arche_id) };
                self.entities = arche.entities();
                if !self.entities.is_empty() {
                    let table_id = arche.table_id();
                    let storages = unsafe { &self.world.read_only().storages };
                    let table = unsafe { storages.tables.get_unchecked(table_id) };
                    unsafe {
                        D::set_for_arche(&self.state.d_state, &mut self.d_cache, arche, table);
                        F::set_for_arche(&self.state.f_state, &mut self.f_cache, arche, table);
                    }
                    return Some(());
                }
            }
        }
    }
}

impl<'w, D: QueryData, F: QueryFilter> Iterator for QueryIter<'w, '_, D, F> {
    type Item = D::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        'looper: loop {
            if self.row >= self.entities.len() {
                // If there is no entities, `update_slice` will return None.
                // otherwise `self.entities` is not empty after this function.
                self.update_slice()?;
            }
            // we cannot storage old_row before `update_slice`,
            // because it will reset `self.row` always.
            let old_row = self.row;

            let entity = unsafe { *self.entities.get_unchecked(old_row) };
            // the number of entities < u32::MAX, the row will never overflow.
            self.row += 1;

            let table_row = if QueryState::<D, F>::IS_DENSE {
                TableRow(old_row as u32)
            } else {
                let infos = unsafe { &self.world.read_only().entities };
                infos.locate(entity).unwrap().table_row
            };

            // Important optimization: skip entity filtering when the filter
            // type guarantees no entity-level checks are needed.
            if F::ENABLE_ENTITY_FILTER {
                let f_state = &self.state.f_state;
                let f_cache = &mut self.f_cache;
                if unsafe { !F::filter(f_state, f_cache, entity, table_row) } {
                    continue 'looper;
                }
            }

            let d_state = &self.state.d_state;
            let d_cache = &mut self.d_cache;
            if let Some(data) = unsafe { D::fetch(d_state, d_cache, entity, table_row) } {
                return Some(data);
            }
        }
    }
}

impl<D: QueryData, F: QueryFilter> FusedIterator for QueryIter<'_, '_, D, F> {}

// -----------------------------------------------------------------------------
// Query -> QueryIter

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

impl<'s, D: QueryData, F: QueryFilter> Query<'_, 's, D, F> {
    /// Returns a mutable iterator over query results.
    pub fn iter_mut(&mut self) -> QueryIter<'_, 's, D, F> {
        unsafe { QueryIter::new(self.world, self.state, self.last_run, self.this_run) }
    }

    /// Returns a read-only iterator over query results.
    pub fn iter(&self) -> QueryIter<'_, 's, D, F>
    where
        D: ReadOnlyQueryData,
    {
        unsafe { QueryIter::new(self.world, self.state, self.last_run, self.this_run) }
    }
}

// -----------------------------------------------------------------------------
// QueryState -> QueryIter

impl<D: QueryData, F: QueryFilter> QueryState<D, F> {
    /// Creates a mutable iterator from this query state and world.
    pub fn iter_mut<'s, 'w>(&'s self, world: &'w mut World) -> QueryIter<'w, 's, D, F> {
        let last_run = world.last_run();
        let this_run = world.this_run();
        let world = world.unsafe_world();
        unsafe { QueryIter::new(world, self, last_run, this_run) }
    }

    /// Creates a read-only iterator from this query state and world.
    pub fn iter<'s, 'w>(&'s self, world: &'w World) -> QueryIter<'w, 's, D, F>
    where
        D: ReadOnlyQueryData,
    {
        let last_run = world.last_run();
        let this_run = world.this_run();
        let world = world.unsafe_world();
        unsafe { QueryIter::new(world, self, last_run, this_run) }
    }
}
