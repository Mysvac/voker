use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::fmt::Debug;

use voker_utils::hash::NoOpHashSet;

use crate::archetype::{ArcheId, Archetypes};
use crate::entity::StorageId;
use crate::query::{QueryData, QueryFilter};
use crate::resource::Resource;
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
            .field("world_id", &self.world_id)
            .field("storages", &self.storages)
            .field("is_dense", &Self::IS_DENSE)
            .field("filter_date", &self.filter_data)
            .field("filter_params", &self.filter_params)
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
        panic!("invalid query data: {}", DebugName::type_name::<D>())
    }

    /// Builds a new query state from the given world.
    ///
    /// This initializes query/filter internal states, computes filter params,
    /// and collects the initial matched storage set.
    pub fn new(world: &mut World) -> Self {
        let world_id = world.id();
        let version = world.archetypes.len();

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

        let storages: Vec<StorageId> = if Self::IS_DENSE {
            collect_tables(&filter_params, &world.archetypes)
        } else {
            collect_arches(&filter_params, &world.archetypes)
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

        let archetypes = &world.archetypes;
        if archetypes.len() > self.version {
            if Self::IS_DENSE {
                updata_dense_state(
                    &mut self.version,
                    &mut self.storages,
                    &self.filter_params,
                    archetypes,
                );
            } else {
                updata_sparse_state(
                    &mut self.version,
                    &mut self.storages,
                    &self.filter_params,
                    archetypes,
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
fn updata_dense_state(
    version: &mut usize,
    storages: &mut Vec<StorageId>,
    filter_params: &[FilterParam],
    archetypes: &Archetypes,
) {
    let old_len = storages.len();
    let new_version = archetypes.len();

    let mut buffer: Vec<StorageId> = Vec::with_capacity(new_version - old_len);

    for arche_id in (*version)..new_version {
        let arche_id = unsafe { ArcheId::new_unchecked(arche_id as u32) };
        let archetype = unsafe { archetypes.get_unchecked(arche_id) };
        let storage_id = StorageId {
            table_id: archetype.table_id(),
        };

        if storages.binary_search(&storage_id).is_err() {
            let matched = filter_params
                .iter()
                .any(|param| archetype.matches_sorted(param.with(), param.without()));

            if matched {
                buffer.push(storage_id);
            }
        }
    }

    // Assuming the max table ID in storages is A, which represents
    // the maximum ID that met the criteria during the last update.
    //
    // Obviously, we only need to determine whether the newly added
    // table meets the conditions. An important discovery is that,
    // the table ID that meets the conditions and is not in storages
    // must have a value greater than `A`. If its value is less than
    // or equal to A, then this table must have been processed in a
    // previous update, If the conditions are met, it should already
    // be in storages.
    //
    // Therefore, we only need to sort the newly added parts.

    buffer.sort_unstable();
    buffer.dedup();
    storages.append(&mut buffer);

    *version = new_version;
}

#[inline(never)]
fn updata_sparse_state(
    version: &mut usize,
    storages: &mut Vec<StorageId>,
    filter_params: &[FilterParam],
    archetypes: &Archetypes,
) {
    let new_version = archetypes.len();

    for arche_id in (*version)..new_version {
        let arche_id = unsafe { ArcheId::new_unchecked(arche_id as u32) };
        let archetype = unsafe { archetypes.get_unchecked(arche_id) };

        let matched = filter_params
            .iter()
            .any(|param| archetype.matches_sorted(param.with(), param.without()));
        if matched {
            storages.push(StorageId { arche_id });
        }
    }

    // The pushed arche_ids are already sorted.

    *version = new_version;
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
fn collect_arches(params: &[FilterParam], archetypes: &Archetypes) -> Vec<StorageId> {
    // N: the number of archetypes
    // M: the average number of components in an achetype
    // X: the number of filter_params
    // Y: the average number of components in a filter_param
    // Then:
    // Collect From ArcheFilter: X * Y * F(N, M), F == ??
    // Collect From Each Arche : X * Y * N * log M
    let arche_filter = archetypes.filter();

    // We hope the results are in order.
    let mut collector = BTreeSet::<StorageId>::new();

    params.iter().for_each(|param| {
        // default filter without any contents,
        // so it's Clone is cheap (only stack copy).
        let mut filter = arche_filter.clone();
        param.with().iter().for_each(|id| {
            filter.with(*id);
        });
        param.without().iter().for_each(|id| {
            filter.without(*id);
        });
        // ↓ collect_arche, instead of collect_table
        filter.collect_arche(&mut collector);
    });

    collector.into_iter().collect()
}

#[inline(never)]
fn collect_tables(params: &[FilterParam], archetypes: &Archetypes) -> Vec<StorageId> {
    let arche_filter = archetypes.filter();
    let mut collector = BTreeSet::<StorageId>::new();

    params.iter().for_each(|param| {
        let mut filter = arche_filter.clone();
        param.with().iter().for_each(|id| {
            filter.with(*id);
        });
        param.without().iter().for_each(|id| {
            filter.without(*id);
        });
        // ↓ collect_table, instead of collect_arche
        filter.collect_table(&mut collector);
    });

    collector.into_iter().collect()
}
