use core::fmt::Debug;

use voker_utils::hash::SparseHashSet;

use crate::component::ComponentId;

/// Component-level access summary for one logical query path.
///
/// # Design role
///
/// `AccessParam` models read/write access against component ids and entity
/// handles (`EntityRef` / `EntityMut`) before values are merged into a full
/// [`AccessTable`](super::AccessTable).
///
/// # Rule model
///
/// - shared entity access (`entity_ref`) is incompatible with component writes,
/// - exclusive entity access (`entity_mut`) is incompatible with any other
///   component or entity access,
/// - component writes imply component reads for conflict checks.
///
/// These rules are enforced by `can_*` + `set_*` pairs.
#[derive(Default, Clone)]
pub struct AccessParam {
    entity_mut: bool, // holding `EntityMut`
    entity_ref: bool, // holding `EntityRef`
    reading: SparseHashSet<ComponentId>,
    writing: SparseHashSet<ComponentId>,
}

impl Debug for AccessParam {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.entity_mut || self.entity_ref {
            f.debug_struct("AccessParam")
                .field("entity_mut", &self.entity_mut)
                .field("entity_ref", &self.entity_ref)
                .finish()
        } else {
            f.debug_struct("AccessParam")
                .field("reading", &self.reading)
                .field("writing", &self.writing)
                .finish()
        }
    }
}

impl AccessParam {
    /// Creates an empty access summary.
    pub const fn new() -> Self {
        Self {
            entity_mut: false,
            entity_ref: false,
            reading: SparseHashSet::new(),
            writing: SparseHashSet::new(),
        }
    }

    /// Returns whether shared-entity access can be added.
    ///
    /// Shared entity access is allowed only when no exclusive-entity access and
    /// no component writes have been declared.
    pub fn can_entity_ref(&self) -> bool {
        !self.entity_mut && self.writing.is_empty()
    }

    /// Returns whether exclusive-entity access can be added.
    ///
    /// This requires a fully empty access state.
    pub fn can_entity_mut(&self) -> bool {
        !self.entity_mut && !self.entity_ref && self.reading.is_empty() && self.writing.is_empty()
    }

    /// Returns whether component read access can be added for `id`.
    pub fn can_reading(&self, id: ComponentId) -> bool {
        self.entity_ref || (!self.entity_mut && !self.writing.contains(&id))
    }

    /// Returns whether component write access can be added for `id`.
    pub fn can_writing(&self, id: ComponentId) -> bool {
        !self.entity_mut && !self.entity_ref && !self.reading.contains(&id)
    }

    /// Declares shared-entity access.
    #[must_use]
    pub fn set_entity_ref(&mut self) -> bool {
        if self.can_entity_ref() {
            self.entity_ref = true;
            self.reading = SparseHashSet::new();
            true
        } else {
            voker_utils::cold_path();
            false
        }
    }

    /// Declares exclusive-entity access.
    #[must_use]
    pub fn set_entity_mut(&mut self) -> bool {
        if self.can_entity_mut() {
            self.entity_mut = true;
            self.reading = SparseHashSet::new();
            self.writing = SparseHashSet::new();
            true
        } else {
            voker_utils::cold_path();
            false
        }
    }

    /// Declares component read access.
    #[must_use]
    pub fn set_reading(&mut self, id: ComponentId) -> bool {
        if self.can_reading(id) {
            if !self.entity_ref {
                self.reading.insert(id);
            }
            true
        } else {
            voker_utils::cold_path();
            false
        }
    }

    /// Declares component write access.
    ///
    /// Writes are also inserted into `reading` so later checks can use simple
    /// read/write-set disjointness rules.
    #[must_use]
    pub fn set_writing(&mut self, id: ComponentId) -> bool {
        if self.can_writing(id) {
            self.reading.insert(id);
            self.writing.insert(id);
            true
        } else {
            voker_utils::cold_path();
            false
        }
    }

    /// Returns whether this access summary is read-only.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.entity_ref || (!self.entity_mut && self.writing.is_empty())
    }

    /// Returns whether this access can run in parallel with `other`.
    #[must_use]
    pub fn parallelizable(&self, other: &Self) -> bool {
        if self.entity_mut || other.entity_mut {
            return false;
        }
        if self.entity_ref {
            return other.writing.is_empty();
        }
        if other.entity_ref {
            return self.writing.is_empty();
        }
        self.writing.is_disjoint(&other.reading) && other.writing.is_disjoint(&self.reading)
    }

    /// Adds filter-required read access without normal mutability checks.
    ///
    /// This is used by read-only query filters (for example change-detection
    /// style filters) that need metadata reads not explicitly present in query
    /// data fields.
    ///
    /// Ordering requirement:
    /// - regular query-data access should be registered first,
    /// - `force_reading` should be applied afterward for filter-side metadata.
    ///
    /// This method intentionally has no `force_writing` counterpart.
    pub fn force_reading(&mut self, id: ComponentId) {
        if !self.entity_mut && !self.entity_ref {
            self.reading.insert(id);
        }
    }

    /// Merges `other` access into `self`.
    ///
    /// This is used when multiple parameters are composed into one system-level
    /// query bucket.
    pub fn merge_with(&mut self, other: &Self) {
        self.entity_mut |= other.entity_mut;
        self.entity_ref &= other.entity_ref;
        if self.entity_mut || self.entity_ref {
            self.writing = SparseHashSet::new();
            self.reading = SparseHashSet::new();
        } else {
            self.reading.extend(&other.reading);
            self.writing.extend(&other.writing);
        }
    }
}
