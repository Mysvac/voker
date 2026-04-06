use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;

use voker_utils::hash::NoOpHashSet;

use super::{QueryData, QueryFilter};
use crate::archetype::{ArcheId, Archetypes};
use crate::component::ComponentId;
use crate::entity::StorageId;
use crate::resource::Resource;
use crate::storage::{TableId, Tables};
use crate::system::{AccessParam, AccessTable, FilterParam, FilterParamBuilder};
use crate::utils::DebugName;
use crate::world::{World, WorldId};

// -----------------------------------------------------------------------------
// QueryState

/// Reusable query state for a specific query type.
///
/// `QueryState` roughly contains:
/// - The owning world ID
/// - A state version used for incremental updates
/// - The set of matched archetypes or tables at the current version
/// - Cached state for query data and query filters
///
/// # Incremental Updates
///
/// As described in [`Query`], query filtering happens in two phases:
/// archetype filtering and entity filtering. [`QueryState`] caches the
/// archetype-filtering result.
///
/// If a query involves sparse components, the archetype-filtering output is an
/// archetype set (by [`ArcheId`]). If the query is fully dense, the cached
/// output is a table set.
///
/// In `World`, archetype count only grows and never shrinks, and each generated
/// archetype represents a fixed component set. Therefore, the archetype count
/// is used as a version number, and updates only need to process newly added
/// archetypes.
///
/// # Usage
///
/// [`Query`] is effectively a typed view over [`QueryState`]. In most contexts,
/// operations that work with [`Query`] can also be performed directly with
/// [`QueryState`], such as iterating with `iter_mut`.
///
/// [`Query`]: crate::query::Query
///
/// # World Affinity
///
/// A `QueryState` is bound to the world it was built from.
/// Reusing it with another world is invalid and guarded by runtime checks.
#[derive(Clone)]
pub struct QueryState<D: QueryData, F: QueryFilter = ()> {
    pub(super) world_id: WorldId,
    pub(super) version: usize,
    pub(super) storages: Vec<StorageId>,
    pub(super) filter_data: AccessParam,
    pub(super) filter_params: Box<[FilterParam]>,
    pub(super) d_state: D::State,
    pub(super) f_state: F::State,
}

impl<D: QueryData + 'static, F: QueryFilter + 'static> Resource for QueryState<D, F> {}

impl<D: QueryData, F: QueryFilter> Debug for QueryState<D, F> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("QueryState")
            .field("name", &DebugName::type_name::<Self>())
            .field("world_id", &self.world_id)
            .field("storages", &self.storages)
            .field("is_dense", &Self::IS_DENSE)
            .finish_non_exhaustive()
    }
}

impl<D: QueryData, F: QueryFilter> QueryState<D, F> {
    /// Compile-time flag indicating whether this query is fully dense.
    ///
    /// `true` means neither query data nor query filters involve sparse
    /// components, so table-based caching can be used.
    pub const IS_DENSE: bool = D::COMPONENTS_ARE_DENSE && F::COMPONENTS_ARE_DENSE;

    /// Returns the world ID this query state belongs to.
    pub fn world_id(&self) -> WorldId {
        self.world_id
    }

    fn invalid_query_data() -> ! {
        panic! {
            "invalid query data `{}` in query `{}`",
            DebugName::type_name::<D>(),
            DebugName::type_name::<Self>(),
        }
    }

    /// Builds a new query state from the given world.
    ///
    /// This initializes query/filter internal states, computes filter params,
    /// and collects the initial matched storage set.
    pub fn new(world: &mut World) -> Self {
        let world_id = world.id();

        let d_state = D::build_state(world);
        let f_state = F::build_state(world);

        let mut filter_data = AccessParam::new();
        if !D::build_access(&d_state, &mut filter_data) {
            Self::invalid_query_data();
        } // `F::build_access` function must be called after `D::build_access`.
        F::build_access(&f_state, &mut filter_data);

        let mut builders = Vec::<FilterParamBuilder>::new();
        // `F::build_filter` function must be called before `D::build_filter`.
        F::build_filter(&f_state, &mut builders);
        D::build_filter(&d_state, &mut builders);
        let filter_params: Box<[FilterParam]> = collect_param(builders);

        let mut version: usize = 0;
        let mut storages: Vec<StorageId> = Vec::new();

        if Self::IS_DENSE {
            let tables = &world.storages.tables;
            let size_hint = (tables.len() >> 2).next_power_of_two() >> 1;
            storages.reserve(size_hint);
            updata_table_state(&mut version, &mut storages, &filter_params, tables);
        } else {
            let arches = &world.archetypes;
            let size_hint = (arches.len() >> 2).next_power_of_two() >> 1;
            storages.reserve(size_hint);
            updata_arche_state(&mut version, &mut storages, &filter_params, arches);
        };

        QueryState {
            world_id,
            version,
            storages,
            filter_data,
            filter_params,
            d_state,
            f_state,
        }
    }

    /// Incrementally updates cached storage matches against the current world.
    ///
    /// Only archetypes added since the last recorded version are processed.
    /// Panics if `world` does not match [`QueryState::world_id`].
    pub fn update(&mut self, world: &World) {
        assert!(self.world_id == world.id());

        if Self::IS_DENSE {
            let tables = &world.storages.tables;
            if tables.len() > self.version {
                updata_table_state(
                    &mut self.version,
                    &mut self.storages,
                    &self.filter_params,
                    tables,
                );
            }
        } else {
            let arches = &world.archetypes;
            if arches.len() > self.version {
                updata_arche_state(
                    &mut self.version,
                    &mut self.storages,
                    &self.filter_params,
                    arches,
                );
            }
        }
    }

    /// Records this query's access requirements into an [`AccessTable`].
    ///
    /// Returns `false` when access conflicts are detected.
    pub(crate) fn mark_assess(&self, access_table: &mut AccessTable) -> bool {
        let data: &AccessParam = &self.filter_data;
        let params: &[FilterParam] = &self.filter_params;
        access_table.set_query(data, params)
    }
}

#[inline(never)]
fn collect_param(builders: Vec<FilterParamBuilder>) -> Box<[FilterParam]> {
    // We use NoOpHash because FilterParam is pre-hased.
    let mut params: NoOpHashSet<FilterParam> = NoOpHashSet::with_capacity(builders.len());
    builders.into_iter().for_each(|builder| {
        if let Some(param) = builder.build() {
            params.insert(param);
        }
    });

    params.into_iter().collect()
}

#[inline(never)]
fn updata_table_state(
    version: &mut usize,
    storages: &mut Vec<StorageId>,
    filter_params: &[FilterParam],
    tables: &Tables,
) {
    let new_version = tables.len();

    for table_id in (*version)..new_version {
        let table_id = unsafe { TableId::new_unchecked(table_id as u32) };
        let table = unsafe { tables.get_unchecked(table_id) };
        let dense = table.components();

        let matched = filter_params
            .iter()
            .any(|p| matches_sorted(dense, &[], p.with(), p.without()));

        if matched {
            storages.push(StorageId { table_id });
        }
    }

    // The pushed table_ids are already sorted.
    *version = new_version;
}

#[inline(never)]
fn updata_arche_state(
    version: &mut usize,
    storages: &mut Vec<StorageId>,
    filter_params: &[FilterParam],
    arches: &Archetypes,
) {
    let new_version = arches.len();

    for arche_id in (*version)..new_version {
        let arche_id = unsafe { ArcheId::new_unchecked(arche_id as u32) };
        let arche = unsafe { arches.get_unchecked(arche_id) };
        let dense = arche.dense_components();
        let sparse = arche.sparse_components();

        let matched = filter_params
            .iter()
            .any(|p| matches_sorted(dense, sparse, p.with(), p.without()));

        if matched {
            storages.push(StorageId { arche_id });
        }
    }

    // The pushed arche_ids are already sorted.
    *version = new_version;
}

/// Fast archetype matching requiring sorted input slices.
///
/// # Complexity
/// - Time: O(min(m + n, m * log n)) where m = len(with) + len(without), n = total components
/// - Space: O(1)
#[inline]
fn matches_sorted(
    dense: &[ComponentId],
    sparse: &[ComponentId],
    with: &[ComponentId],
    without: &[ComponentId],
) -> bool {
    fn jump_search(id: ComponentId, slice: &[ComponentId]) -> Result<usize, usize> {
        let mut index = 0usize;
        let len = slice.len();

        loop {
            if index == len || slice[index] > id {
                return Err(index);
            }
            if slice[index] == id {
                return Ok(index);
            }

            let mut step = 1usize;
            loop {
                let offset = index + step;
                if offset < len && slice[offset] <= id {
                    step <<= 1;
                } else {
                    break;
                }
            }
            // index + (step >> 1) < len
            // index + max(step >> 1, 1) <= len
            index += core::cmp::max(step >> 1, 1);
        }
    }

    {
        // with
        let mut dense = dense;
        let mut sparse = sparse;
        let result = with.iter().all(|&id| {
            // `with` has been sorted and deduplicated, the `[..=idx]` can be skipped.
            // we can skip `[idx]` because it's `==` specific id.
            if let Ok(idx) = jump_search(id, dense) {
                dense = &dense[(idx + 1)..];
                return true;
            }
            if let Ok(idx) = jump_search(id, sparse) {
                sparse = &sparse[(idx + 1)..];
                return true;
            }
            false
        });
        if !result {
            return false;
        }
    }
    {
        // without
        let mut dense = dense;
        let mut sparse = sparse;
        // `without` has been sorted and deduplicated, the `[..idx]` can be skipped.
        // cannot skip `[idx]` because it's `>` specific id (or the end of slice).
        without.iter().all(|&id| {
            if let Err(idx) = jump_search(id, dense) {
                dense = &dense[idx..];
            } else {
                return false;
            }
            if let Err(idx) = jump_search(id, sparse) {
                sparse = &sparse[idx..];
            } else {
                return false;
            }
            true
        })
    }
}
