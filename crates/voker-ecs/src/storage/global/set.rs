use alloc::vec::Vec;
use core::fmt::Debug;
use core::iter::FusedIterator;

use super::ResData;
use crate::resource::{ResourceId, ResourceInfo};
use crate::storage::AbortOnPanic;
use crate::tick::Tick;
use crate::utils::DebugCheckedUnwrap;

// -----------------------------------------------------------------------------
// ResSet

/// A collection of all resources in the world.
///
/// Provides indexed access to resources by their [`ResourceId`] with
/// O(1) lookup through a sparse index map.
pub struct ResSet {
    data: Vec<Option<ResData>>,
}

// -----------------------------------------------------------------------------
// Private

impl ResSet {
    /// Creates a new empty resource collection.
    #[inline]
    pub(crate) const fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Updates all resource ticks to prevent overflow.
    pub(crate) fn check_ticks(&mut self, now: Tick) {
        self.data.iter_mut().for_each(|data| {
            if let Some(data) = data {
                data.check_ticks(now);
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

        if self.data.get(info.id().index()).is_none_or(Option::is_none) {
            prepare_internal(self, info);
        }
    }
}

// -----------------------------------------------------------------------------
// Basic

impl Debug for ResSet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list()
            .entries(self.data.iter().filter_map(Option::as_ref))
            .finish()
    }
}

impl ResSet {
    /// Returns the number of Maps.
    #[inline]
    #[expect(clippy::len_without_is_empty, reason = "consistency")]
    pub fn len(&self) -> usize {
        self.data.iter().filter_map(Option::as_ref).count()
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
    #[inline(always)]
    pub unsafe fn get_unchecked(&self, id: ResourceId) -> &ResData {
        debug_assert!(id.index() < self.data.len());
        unsafe { self.data.get_unchecked(id.index()).as_ref().debug_checked_unwrap() }
    }

    /// Returns a mutable reference to the resource data for the given ID.
    ///
    /// # Safety
    /// - The caller must ensure the resource is prepared (instead of registered).
    #[inline(always)]
    pub unsafe fn get_unchecked_mut(&mut self, id: ResourceId) -> &mut ResData {
        debug_assert!(id.index() < self.data.len());
        unsafe {
            self.data
                .get_unchecked_mut(id.index())
                .as_mut()
                .debug_checked_unwrap()
        }
    }

    /// Returns an iterator over the resources.
    #[inline]
    pub fn iter(&self) -> impl FusedIterator<Item = &'_ ResData> {
        self.data.iter().filter_map(Option::as_ref)
    }

    /// Returns an iterator that allows modifying each resource.
    #[inline]
    pub fn iter_mut(&mut self) -> impl FusedIterator<Item = &'_ mut ResData> {
        self.data.iter_mut().filter_map(Option::as_mut)
    }
}
