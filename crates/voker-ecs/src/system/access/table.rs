use core::fmt::Debug;

use fixedbitset::FixedBitSet;
use voker_utils::hash::NoopHashMap;

use super::{AccessParam, FilterParam};
use crate::resource::ResourceId;

/// Full per-system access declaration used by scheduler conflict checks.
///
/// # Design pattern
///
/// `AccessTable` combines three access domains:
/// 1. world-level access (`&World` / `&mut World`),
/// 2. resource-level read/write sets,
/// 3. query-level component access grouped by [`FilterParam`].
///
/// Grouping query access by filter keys enables a stricter but less pessimistic
/// conflict test: mutable access to the same component may still be parallel if
/// filter constraints prove disjoint entity sets.
///
/// # Rule matrix (same table)
///
/// - world mut vs anything: conflict
/// - world ref vs world ref: compatible
/// - world ref vs resource/query write: conflict
/// - resource read vs resource read: compatible
/// - resource read vs resource write: conflict
/// - resource write vs resource write: conflict
/// - query access: compatible only when each overlapping filter bucket has
///   [`AccessParam::parallelizable`] access sets.
#[derive(Default)]
pub struct AccessTable {
    world_mut: bool,          // holding `&mut world`
    world_ref: bool,          // holding `&world`
    res_reading: FixedBitSet, // resource reading
    res_writing: FixedBitSet, // resource writing
    filter: NoopHashMap<FilterParam, AccessParam>,
}

// `#[derive(Clone)]` does not generate optimized `clone_from`.
impl Clone for AccessTable {
    fn clone(&self) -> Self {
        Self {
            world_mut: self.world_mut,
            world_ref: self.world_ref,
            res_reading: self.res_reading.clone(),
            res_writing: self.res_writing.clone(),
            filter: self.filter.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.world_mut = source.world_mut;
        self.world_ref = source.world_ref;
        self.res_reading.clone_from(&source.res_reading);
        self.res_writing.clone_from(&source.res_writing);
        self.filter.clone_from(&source.filter);
    }
}

impl Debug for AccessTable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        struct FormattedBitSet<'a>(&'a FixedBitSet);
        impl Debug for FormattedBitSet<'_> {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_list().entries(self.0.ones()).finish()
            }
        }

        f.debug_struct("AccessTable")
            .field("world_mut", &self.world_mut)
            .field("world_ref", &self.world_ref)
            .field("res_reading", &FormattedBitSet(&self.res_reading))
            .field("res_writing", &FormattedBitSet(&self.res_writing))
            .finish()
    }
}

impl AccessTable {
    /// Creates an empty [`AccessTable`] collection.
    pub const fn new() -> Self {
        Self {
            world_mut: false,
            world_ref: false,
            res_reading: FixedBitSet::new(),
            res_writing: FixedBitSet::new(),
            filter: NoopHashMap::new(),
        }
    }

    /// Returns whether exclusive world access can be declared.
    ///
    /// Can only be used for table building, invalid for merged table.
    fn can_world_mut(&self) -> bool {
        !self.world_mut
            && !self.world_ref
            && self.res_reading.is_clear()
            && self.res_writing.is_clear()
            && self.filter.is_empty()
    }

    /// Returns whether shared world access can be declared.
    ///
    /// Can only be used for table building, invalid for merged table.
    fn can_world_ref(&self) -> bool {
        self.world_ref || {
            !self.world_mut
                && self.res_writing.is_clear()
                && self.filter.values().all(AccessParam::is_read_only)
        }
    }

    /// Declares exclusive world access.
    ///
    /// Can only be used for table building, invalid for merged table.
    pub fn set_world_mut(&mut self) -> bool {
        if self.can_world_mut() {
            *self = const { Self::new() };
            self.world_mut = true;
            true
        } else {
            core::hint::cold_path();
            false
        }
    }

    /// Declares shared world access.
    ///
    /// Can only be used for table building, invalid for merged table.
    pub fn set_world_ref(&mut self) -> bool {
        if self.can_world_ref() {
            if !self.world_ref {
                *self = const { Self::new() };
                self.world_ref = true;
            }
            true
        } else {
            core::hint::cold_path();
            false
        }
    }

    /// Returns whether read access to a resource id can be declared.
    ///
    /// Can only be used for table building, invalid for merged table.
    fn can_reading_res(&self, id: ResourceId) -> bool {
        self.world_ref || (!self.world_mut && !self.res_writing.contains(id.index()))
    }

    /// Returns whether write access to a resource id can be declared.
    ///
    /// Can only be used for table building, invalid for merged table.
    fn can_writing_res(&self, id: ResourceId) -> bool {
        !self.world_ref && !self.world_mut && !self.res_reading.contains(id.index())
    }

    /// Declares read access to a resource id.
    ///
    /// Can only be used for table building, invalid for merged table.
    pub fn set_reading_res(&mut self, id: ResourceId) -> bool {
        if self.can_reading_res(id) {
            if !self.world_ref {
                self.res_reading.grow_and_insert(id.index());
            }
            true
        } else {
            core::hint::cold_path();
            false
        }
    }

    /// Declares write access to a resource id.
    ///
    /// Can only be used for table building, invalid for merged table.
    pub fn set_writing_res(&mut self, id: ResourceId) -> bool {
        if self.can_writing_res(id) {
            let index = id.index();
            self.res_reading.grow_and_insert(index);
            self.res_writing.grow_and_insert(index);
            true
        } else {
            core::hint::cold_path();
            false
        }
    }

    /// Returns whether query access can be registered for all filter buckets.
    ///
    /// Query conflicts are checked per filter bucket. If two filters are
    /// disjoint, the corresponding access sets do not need to be compared.
    pub fn can_query(&self, data: &AccessParam, params: &[FilterParam]) -> bool {
        if self.world_mut {
            return false;
        }
        if self.world_ref {
            return data.is_read_only();
        }
        params.iter().all(|param| {
            self.filter.iter().all(|(k, v)| {
                if k.is_disjoint(param) {
                    true
                } else {
                    data.parallelizable(v)
                }
            })
        })
    }

    /// Registers query access into all provided filter buckets.
    ///
    /// Existing buckets are merged to support multiple parameters mapping to
    /// the same logical filter key.
    pub fn set_query(&mut self, data: &AccessParam, params: &[FilterParam]) -> bool {
        if self.can_query(data, params) {
            if !self.world_ref {
                params.iter().for_each(|param| {
                    if let Some(item) = self.filter.get_mut(param) {
                        item.merge_with(data);
                    } else {
                        self.filter.insert(param.clone(), data.clone());
                    }
                });
            }
            true
        } else {
            false
        }
    }

    /// Returns whether two full system access tables are parallel-compatible.
    ///
    /// This method is the scheduler-facing predicate used to build conflict
    /// graphs between systems.
    pub fn parallelizable(&self, other: &Self) -> bool {
        if self.world_mut || other.world_mut {
            return false;
        }
        if !self.res_writing.is_disjoint(&other.res_reading)
            || !other.res_writing.is_disjoint(&self.res_reading)
        {
            return false;
        }
        self.filter.iter().all(|(k, v)| {
            other.filter.iter().all(|(x, y)| {
                if k.is_disjoint(x) {
                    true
                } else {
                    v.parallelizable(y)
                }
            })
        })
    }

    /// Merges two access tables conservatively.
    ///
    /// Used when composing systems (for example `pipe` combinators) into one
    /// executable unit with a single access declaration.
    pub fn merge(mut self, other: Self) -> Self {
        self.world_mut |= other.world_mut;
        self.world_ref |= other.world_ref;
        if self.world_mut {
            self.res_reading = FixedBitSet::new();
            self.res_writing = FixedBitSet::new();
            self.filter = NoopHashMap::new();
        } else {
            self.res_reading.union_with(&other.res_reading);
            self.res_writing.union_with(&other.res_writing);
            other.filter.into_iter().for_each(|(param, data)| {
                if let Some(item) = self.filter.get_mut(&param) {
                    item.merge_with(&data);
                } else {
                    self.filter.insert(param, data);
                }
            });
        }
        self
    }
}
