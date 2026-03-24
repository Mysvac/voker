use super::ResData;
use crate::resource::{ResourceId, ResourceInfo};
use crate::storage::AbortOnPanic;
use crate::tick::{CheckTicks, Tick};
use crate::utils::DebugCheckedUnwrap;
use alloc::vec::Vec;
use core::fmt::Debug;

/// A collection of all resources in the world.
///
/// Provides indexed access to resources by their [`ResourceId`] with
/// O(1) lookup through a sparse index map.
pub struct ResSet {
    data: Vec<Option<ResData>>,
}

unsafe impl Send for ResSet {}
unsafe impl Sync for ResSet {}

impl Debug for ResSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map()
            .entries(
                self.data
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, v)| v.as_ref().map(|v| (idx, v))),
            )
            .finish()
    }
}

impl ResSet {
    /// Creates a new empty resource collection.
    #[inline]
    pub(crate) const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Check whether a certain resource has been registered
    ///
    /// Having registered indicates that we have allocated space for it,
    /// and `get` must return `Some(&ResData)`, but the specific data may
    /// still be uninitialized.
    #[inline]
    pub fn contains(&self, id: ResourceId) -> bool {
        self.data.get(id.index()).is_some_and(Option::is_some)
    }

    /// Returns a shared reference to the resource data for the given ID, if it exists.
    #[inline]
    pub fn get(&self, id: ResourceId) -> Option<&ResData> {
        self.data.get(id.index()).and_then(Option::as_ref)
    }

    /// Returns a mutable reference to the resource data for the given ID, if it exists.
    #[inline]
    pub fn get_mut(&mut self, id: ResourceId) -> Option<&mut ResData> {
        self.data.get_mut(id.index()).and_then(Option::as_mut)
    }

    /// Returns a shared reference to the resource data for the given ID.
    ///
    /// # Safety
    /// - The caller must ensure the resource is prepared (instead of registered)..
    #[inline]
    pub unsafe fn get_unchecked(&self, id: ResourceId) -> &ResData {
        debug_assert!(id.index() < self.data.len());
        unsafe {
            self.data
                .get_unchecked(id.index())
                .as_ref()
                .debug_checked_unwrap()
        }
    }

    /// Returns a mutable reference to the resource data for the given ID.
    ///
    /// # Safety
    /// - The caller must ensure the resource is prepared (instead of registered).
    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, id: ResourceId) -> &mut ResData {
        debug_assert!(id.index() < self.data.len());
        unsafe {
            self.data
                .get_unchecked_mut(id.index())
                .as_mut()
                .debug_checked_unwrap()
        }
    }

    /// Updates all resource ticks to prevent overflow.
    pub(crate) fn check_ticks(&mut self, check: CheckTicks) {
        let now = check.tick();
        let fall_back = now.relative_to(Tick::MAX_AGE);
        self.data.iter_mut().for_each(|data| {
            if let Some(data) = data {
                data.quick_check(now, fall_back);
            }
        });
    }

    /// Prepares storage for a resource if it doesn't already exist.
    ///
    /// This creates the internal [`ResourceData`] for the given component info.
    #[inline]
    pub(crate) fn prepare(&mut self, info: &ResourceInfo) {
        #[cold]
        #[inline(never)]
        fn resize_data(this: &mut ResSet, len: usize) {
            let abort_guard = AbortOnPanic;
            this.data.reserve(len - this.data.len());
            this.data.resize_with(this.data.capacity(), || None);
            ::core::mem::forget(abort_guard);
        }

        #[cold]
        #[inline(never)]
        fn prepare_internal(this: &mut ResSet, info: &ResourceInfo) {
            let id = info.id();
            let index = id.index();
            unsafe {
                if index >= this.data.len() {
                    resize_data(this, index + 1);
                }

                let data = ResData::new(info);
                *this.data.get_unchecked_mut(index) = Some(data);
            }
        }

        if !self.contains(info.id()) {
            prepare_internal(self, info);
        }
    }
}
