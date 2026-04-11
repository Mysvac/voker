use alloc::collections::BTreeSet;
use alloc::vec::Vec;
use core::hash::{Hash, Hasher};
use core::{fmt::Debug, hash::BuildHasher};

use voker_utils::hash::FixedHashState;

use crate::component::ComponentId;

// -----------------------------------------------------------------------------
// FilterParam

/// Builder for constructing query filter parameters.
#[derive(Debug, Default, Clone)]
pub struct FilterParamBuilder {
    // We use BTreeSet to ensure it's ordering.
    with: BTreeSet<ComponentId>,
    without: BTreeSet<ComponentId>,
}

impl FilterParamBuilder {
    pub const fn new() -> Self {
        Self {
            with: BTreeSet::new(),
            without: BTreeSet::new(),
        }
    }

    pub fn with(&mut self, id: ComponentId) {
        self.with.insert(id);
    }

    pub fn without(&mut self, id: ComponentId) {
        self.without.insert(id);
    }

    pub fn merge(&self, other: &Self) -> Option<FilterParamBuilder> {
        if self.with.is_disjoint(&other.without) && other.with.is_disjoint(&self.without) {
            let mut with = self.with.clone();
            with.extend(&other.with);
            let mut without = self.without.clone();
            without.extend(&other.without);
            Some(FilterParamBuilder { with, without })
        } else {
            None
        }
    }

    pub fn build(self) -> Option<FilterParam> {
        use crate::utils::SlicePool;

        if self.with.is_disjoint(&self.without) {
            let with_len = self.with.len();
            let without_len = self.without.len();
            // ComponentId <= u32::MAX, ↓ length overflow is impossible
            let mut vec = Vec::with_capacity(with_len + without_len);
            vec.extend(self.with);
            vec.extend(self.without);

            let params = SlicePool::component(&vec);

            let mut hasher = FixedHashState.build_hasher();
            with_len.hash(&mut hasher);
            params.hash(&mut hasher);
            let hash = hasher.finish();

            debug_assert!(params[..with_len].is_sorted() && params[with_len..].is_sorted());

            Some(FilterParam {
                hash,
                with_len,
                params,
            })
        } else {
            None
        }
    }
}

/// A compact, hashable representation of component filter requirements.
#[derive(Clone, PartialEq, Eq)]
pub struct FilterParam {
    hash: u64,
    with_len: usize,
    params: &'static [ComponentId],
}

impl FilterParam {
    pub fn with(&self) -> &[ComponentId] {
        &self.params[..self.with_len]
    }

    pub fn without(&self) -> &[ComponentId] {
        &self.params[self.with_len..]
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        use core::mem::transmute;
        // `SliceContains` has SIMD optimization for u32
        let x_without = unsafe { transmute::<&[ComponentId], &[u32]>(self.without()) };
        let y_without = unsafe { transmute::<&[ComponentId], &[u32]>(other.without()) };
        let x_with = unsafe { transmute::<&[ComponentId], &[u32]>(self.with()) };
        let y_with = unsafe { transmute::<&[ComponentId], &[u32]>(other.with()) };

        // Although the slice is sorted, we assume the params
        // is usually small, so the `contains` is faster.
        x_without.iter().any(|id| y_with.contains(id))
            || y_without.iter().any(|id| x_with.contains(id))
    }
}

impl Hash for FilterParam {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl Debug for FilterParam {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FilterParam")
            .field("with", &self.with())
            .field("without", &self.without())
            .finish()
    }
}
