use core::fmt::Debug;

use voker_utils::hash::SparseHashSet;

use crate::component::ComponentId;

/// Tracks access patterns during query construction to detect conflicts.
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
    pub const fn new() -> Self {
        Self {
            entity_mut: false,
            entity_ref: false,
            reading: SparseHashSet::new(),
            writing: SparseHashSet::new(),
        }
    }

    pub fn can_entity_ref(&self) -> bool {
        !self.entity_mut && self.writing.is_empty()
    }

    pub fn can_entity_mut(&self) -> bool {
        !self.entity_mut && !self.entity_ref && self.reading.is_empty() && self.writing.is_empty()
    }

    pub fn can_reading(&self, id: ComponentId) -> bool {
        self.entity_ref || (!self.entity_mut && !self.writing.contains(&id))
    }

    pub fn can_writing(&self, id: ComponentId) -> bool {
        !self.entity_mut && !self.entity_ref && !self.reading.contains(&id)
    }

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

    pub fn force_reading(&mut self, id: ComponentId) {
        if !self.entity_mut && !self.entity_ref {
            self.reading.insert(id);
        }
    }

    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.entity_ref || (!self.entity_mut && self.writing.is_empty())
    }

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
