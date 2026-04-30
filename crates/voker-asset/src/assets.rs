use core::any::TypeId;
use core::fmt::Debug;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::Ordering;

use alloc::vec::Vec;

use alloc::sync::Arc;
use thiserror::Error;
use uuid::Uuid;
use voker_ecs::borrow::{Res, ResMut};
use voker_ecs::derive::Resource;
use voker_ecs::prelude::MessageWriter;
use voker_ecs::system::SystemTick;
use voker_utils::hash::HashMap;

use crate::asset::Asset;
use crate::changes::AssetChanges;
use crate::event::AssetEvent;
use crate::handle::{AssetHandleProvider, Handle};
use crate::ident::{AssetId, AssetIndex, AssetIndexAllocator};
use crate::server::AssetServer;

// -----------------------------------------------------------------------------
// AssetTable

struct Entry<A: Asset> {
    value: Option<A>,
    generation: u32,
}

impl<A: Asset> Entry<A> {
    const DEFAULT: Entry<A> = Entry {
        value: None,
        generation: 0,
    };

    #[inline(always)]
    const fn none(generation: u32) -> Self {
        Self {
            value: None,
            generation,
        }
    }
}

struct AssetTable<A: Asset> {
    storage: Vec<Option<Entry<A>>>,
    len: u32,
    allocator: Arc<AssetIndexAllocator>,
}

impl<A: Asset> Default for AssetTable<A> {
    #[inline]
    fn default() -> Self {
        Self {
            len: 0,
            storage: Vec::new(),
            allocator: Arc::new(AssetIndexAllocator::new()),
        }
    }
}

impl<A: Asset> AssetTable<A> {
    pub fn flush(&mut self) {
        let new_len = self.allocator.next_index.load(Ordering::Relaxed);

        self.storage
            .resize_with(new_len as usize, || Some(Entry::<A>::DEFAULT));

        while let Some(recycled) = self.allocator.recycled.pop() {
            let index = recycled.index as usize;
            self.storage[index] = Some(Entry::<A>::none(recycled.generation));
        }
    }

    pub fn get(&self, index: AssetIndex) -> Option<&A> {
        let entry = self.storage.get(index.index as usize)?;
        let Entry { value, generation } = entry.as_ref()?;
        (*generation == index.generation).then_some(value.as_ref())?
    }

    pub fn get_mut(&mut self, index: AssetIndex) -> Option<&mut A> {
        let entry = self.storage.get_mut(index.index as usize)?;
        let Entry { value, generation } = entry.as_mut()?;
        (*generation == index.generation).then_some(value.as_mut())?
    }

    pub fn index_allocator(&self) -> Arc<AssetIndexAllocator> {
        self.allocator.clone()
    }

    // Ok(true) if the old data is replaced, then the len is unchanged.
    // Ok(false) if the new data is inserted, then the len is increased.
    pub fn insert(&mut self, index: AssetIndex, asset: A) -> Result<bool, InvalidAssetGeneration> {
        self.flush();
        let entry = &mut self.storage[index.index as usize];

        let Some(entry) = entry.as_mut() else {
            return Err(InvalidAssetGeneration::Removed { index });
        };

        let Entry { value, generation } = entry;

        if *generation != index.generation {
            return Err(InvalidAssetGeneration::Occupied {
                index,
                current_generation: *generation,
            });
        }

        let is_new_data = value.is_none();
        is_new_data.then(|| {
            self.len += 1;
        });
        *value = Some(asset);
        Ok(!is_new_data)
    }

    pub fn remove_still_alive(&mut self, index: AssetIndex) -> Option<A> {
        self.remove_internal(index, |_| {})
    }

    pub fn remove_and_recycle(&mut self, index: AssetIndex) -> Option<A> {
        self.remove_internal(index, |table| {
            table.storage[index.index as usize] = None;
            table.allocator.recycle(index);
        })
    }

    fn remove_internal(
        &mut self,
        index: AssetIndex,
        removed_action: impl FnOnce(&mut Self),
    ) -> Option<A> {
        self.flush();

        let entry = self.storage[index.index as usize].as_mut()?;
        let Entry { value, generation } = entry;

        if *generation != index.generation {
            return None;
        }

        let value = value.take();
        value.is_some().then(|| {
            self.len -= 1;
        });
        removed_action(self);
        value
    }
}

/// An error returned when an [`AssetIndex`] has an invalid generation.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum InvalidAssetGeneration {
    #[error(
        "AssetIndex {index:?} has an invalid generation. The current generation is: '{current_generation}'."
    )]
    Occupied {
        index: AssetIndex,
        current_generation: u32,
    },
    #[error("AssetIndex {index:?} has been removed")]
    Removed { index: AssetIndex },
}

// -----------------------------------------------------------------------------
// Assets

/// Typed storage for all loaded assets of type `A`.
///
/// `Assets<A>` holds every asset value and drives change detection by queuing
/// [`AssetEvent<A>`] messages whenever an asset is added, modified, removed, or
/// becomes fully loaded.  It is registered as a [`Resource`] when
/// [`app.init_asset::<A>()`](crate::plugin::AppAssetExt::init_asset) is called.
///
/// # Accessing assets
///
/// Use [`get`](Self::get) / [`get_mut`](Self::get_mut) with any type that implements
/// `Into<AssetId<A>>` — including [`Handle<A>`](crate::handle::Handle) and
/// [`AssetId<A>`].
#[derive(Resource)]
pub struct Assets<A: Asset> {
    table: AssetTable<A>,
    hash_map: HashMap<Uuid, A>,
    handle_provider: AssetHandleProvider,
    queued_events: Vec<AssetEvent<A>>,
    duplicate_handles: HashMap<AssetIndex, u16>,
}

impl<A: Asset> Default for Assets<A> {
    fn default() -> Self {
        let table = AssetTable::default();
        let handle_provider = AssetHandleProvider::new(TypeId::of::<A>(), table.index_allocator());
        Self {
            table,
            handle_provider,
            hash_map: Default::default(),
            queued_events: Default::default(),
            duplicate_handles: Default::default(),
        }
    }
}

impl<A: Asset> Assets<A> {
    /// Returns a clone of the [`AssetHandleProvider`] for this asset type.
    pub fn handle_provider(&self) -> AssetHandleProvider {
        self.handle_provider.clone()
    }

    /// Reserves a new strong handle without storing any asset value yet.
    pub fn reserve_handle(&self) -> Handle<A> {
        self.handle_provider.reserve_handle().typed_debug_checked::<A>()
    }

    pub(crate) fn insert_with_uuid(&mut self, uuid: Uuid, asset: A) -> Option<A> {
        let replaced = self.hash_map.insert(uuid, asset);
        if replaced.is_some() {
            self.queued_events.push(AssetEvent::Modified { id: uuid.into() });
        } else {
            self.queued_events.push(AssetEvent::Added { id: uuid.into() });
        }
        replaced
    }

    pub(crate) fn insert_with_index(
        &mut self,
        index: AssetIndex,
        asset: A,
    ) -> Result<bool, InvalidAssetGeneration> {
        let replaced = self.table.insert(index, asset)?;
        if replaced {
            self.queued_events.push(AssetEvent::Modified { id: index.into() });
        } else {
            self.queued_events.push(AssetEvent::Added { id: index.into() });
        }
        Ok(replaced)
    }

    /// Stores `asset` under `id`.
    ///
    /// Returns [`Err`] if `id` is an `Index` variant whose generation does not match the
    /// current slot (the slot was recycled or has already been removed).
    #[inline]
    pub fn insert(
        &mut self,
        id: impl Into<AssetId<A>>,
        asset: A,
    ) -> Result<(), InvalidAssetGeneration> {
        match id.into() {
            AssetId::Index { index, .. } => {
                self.insert_with_index(index, asset)?;
                Ok(())
            }
            AssetId::Uuid { uuid } => {
                self.insert_with_uuid(uuid, asset);
                Ok(())
            }
        }
    }

    /// Inserts `asset` into a freshly allocated slot and returns a strong handle to it.
    ///
    /// Unlike [`insert`](Self::insert), `add` allocates the index automatically and always succeeds.
    #[inline]
    pub fn add(&mut self, asset: impl Into<A>) -> Handle<A> {
        let index = self.table.allocator.reserve();
        self.insert_with_index(index, asset.into()).unwrap();
        Handle::Strong(self.handle_provider.build_handle(index, false, None, None))
    }

    /// Returns `true` if an asset with the given `id` is currently stored.
    #[inline]
    pub fn contains(&self, id: impl Into<AssetId<A>>) -> bool {
        match id.into() {
            AssetId::Index { index, .. } => self.table.get(index).is_some(),
            AssetId::Uuid { uuid } => self.hash_map.contains_key(&uuid),
        }
    }

    /// Creates an additional strong handle for an asset that is already stored.
    ///
    /// Returns [`None`] if the asset does not exist or if the per-slot duplicate-handle
    /// counter has reached its maximum (`u16::MAX`).
    #[inline]
    pub fn resolve_strong_handle(&mut self, id: AssetId<A>) -> Option<Handle<A>> {
        let index = match id {
            AssetId::Index { index, .. } => index,
            // We don't support strong handles for Uuid assets.
            AssetId::Uuid { .. } => return None,
        };

        self.table.get(index)?; // if !contains { return None; }

        let counter = self.duplicate_handles.entry(index).or_insert(0);
        if *counter == u16::MAX {
            core::hint::cold_path();
            tracing::error!(
                "The number of StrongHandle<{}> reach the limit, cannot generate again.",
                core::any::type_name::<A>(),
            );
            return None;
        }

        *counter += 1;

        Some(Handle::Strong(
            self.handle_provider.build_handle(index, false, None, None),
        ))
    }

    /// Returns a mutable iterator over all `(AssetId, &mut A)` pairs.
    ///
    /// Every yielded asset is marked as modified and queues an [`AssetEvent::Modified`].
    #[inline]
    pub fn iter_mut(&mut self) -> AssetsMutIterator<'_, A> {
        AssetsMutIterator {
            queued_events: &mut self.queued_events,
            table: self.table.storage.iter_mut().enumerate(),
            hash_map: self.hash_map.iter_mut(),
        }
    }

    /// Returns an immutable iterator over all `(AssetId, &A)` pairs.
    #[inline]
    pub fn iter(&self) -> AssetsIterator<'_, A> {
        AssetsIterator {
            table: self.table.storage.iter().enumerate(),
            hash_map: self.hash_map.iter(),
        }
    }

    /// Returns an iterator over all [`AssetId<A>`] values currently stored.
    #[inline]
    pub fn iter_id(&self) -> AssetIdIterator<'_, A> {
        AssetIdIterator {
            table: self.table.storage.iter().enumerate(),
            hash_map: self.hash_map.keys(),
        }
    }

    /// Returns `true` if no assets are stored.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.table.len == 0 && self.hash_map.is_empty()
    }

    /// Returns the total number of assets stored (both index-keyed and UUID-keyed).
    #[inline]
    pub fn len(&self) -> usize {
        self.table.len as usize + self.hash_map.len()
    }

    /// Returns a reference to the asset with the given `id`, or [`None`] if not found.
    #[inline]
    pub fn get(&self, id: impl Into<AssetId<A>>) -> Option<&A> {
        match id.into() {
            AssetId::Index { index, .. } => self.table.get(index),
            AssetId::Uuid { uuid } => self.hash_map.get(&uuid),
        }
    }

    /// Returns a mutable reference to the asset without queuing a change event.
    #[inline]
    pub fn get_mut_untracked(&mut self, id: impl Into<AssetId<A>>) -> Option<&mut A> {
        match id.into() {
            AssetId::Index { index, .. } => self.table.get_mut(index),
            AssetId::Uuid { uuid } => self.hash_map.get_mut(&uuid),
        }
    }

    /// Returns a change-tracking mutable reference to the asset.
    ///
    /// An [`AssetEvent::Modified`] is queued when the returned [`AssetMut`] is dropped and
    /// the value was accessed through [`DerefMut`] or
    /// [`into_inner`](AssetMut::into_inner).
    #[inline]
    pub fn get_mut(&mut self, id: impl Into<AssetId<A>>) -> Option<AssetMut<'_, A>> {
        let id: AssetId<A> = id.into();
        let asset = match id {
            AssetId::Index { index, .. } => self.table.get_mut(index),
            AssetId::Uuid { uuid } => self.hash_map.get_mut(&uuid),
        }?;
        let guard = AssetChangeNotifier {
            changed: false,
            asset_id: id,
            queued_events: &mut self.queued_events,
        };
        Some(AssetMut { asset, guard })
    }

    /// Returns a mutable reference to the asset with `id`, inserting the result of
    /// `insert_fn` first if the asset is not yet present.
    pub fn get_or_insert(
        &mut self,
        id: impl Into<AssetId<A>>,
        insert_fn: impl FnOnce() -> A,
    ) -> Result<AssetMut<'_, A>, InvalidAssetGeneration> {
        let id: AssetId<A> = id.into();

        if !self.contains(id) {
            self.insert(id, insert_fn())?;
        }

        Ok(self.get_mut(id).unwrap())
    }

    /// Removes the asset without queuing a change event.
    pub fn remove_untracked(&mut self, id: impl Into<AssetId<A>>) -> Option<A> {
        match id.into() {
            AssetId::Index { index, .. } => {
                self.duplicate_handles.remove(&index);
                self.table.remove_still_alive(index)
            }
            AssetId::Uuid { uuid } => self.hash_map.remove(&uuid),
        }
    }

    /// Removes the asset and queues an [`AssetEvent::Removed`] if it was present.
    pub fn remove(&mut self, id: impl Into<AssetId<A>>) -> Option<A> {
        let id: AssetId<A> = id.into();
        let result = self.remove_untracked(id);
        if result.is_some() {
            self.queued_events.push(AssetEvent::Removed { id });
        }
        result
    }

    pub(crate) fn remove_and_recycle(&mut self, index: AssetIndex) {
        match self.duplicate_handles.get_mut(&index) {
            None => {}
            Some(0) => {
                self.duplicate_handles.remove(&index);
            }
            Some(value) => {
                *value -= 1;
                return;
            }
        }

        let existed = self.table.remove_and_recycle(index).is_some();

        self.queued_events.push(AssetEvent::Unused { id: index.into() });

        if existed {
            self.queued_events.push(AssetEvent::Removed { id: index.into() });
        }
    }

    pub fn track_assets(mut assets: ResMut<Self>, asset_server: Res<AssetServer>) {
        // note that we must hold this lock for the entire duration of this function to ensure
        // that `asset_server.load` calls that occur during it block, which ensures that
        // re-loads are kicked off appropriately. This function must be "transactional" relative
        // to other asset info operations
        let mut infos = asset_server.write_infos();

        while let Ok(drop_event) = assets.handle_provider.drop_receiver.try_recv() {
            if drop_event.asset_server_managed {
                // the process_handle_drop call checks whether new handles have been created since the drop event was fired, before removing the asset
                if !infos.process_handle_drop(drop_event.index) {
                    // a new handle has been created, or the asset doesn't exist
                    continue;
                }
            }

            assets.remove_and_recycle(drop_event.index.index);
        }
    }

    pub(crate) fn handle_asset_events(
        mut assets: ResMut<Self>,
        mut messages: MessageWriter<AssetEvent<A>>,
        asset_changes: Option<ResMut<AssetChanges<A>>>,
        ticks: SystemTick,
    ) {
        use AssetEvent::{Added, FullyLoaded, Modified, Removed, Unused};

        if let Some(mut asset_changes) = asset_changes {
            for new_event in &assets.queued_events {
                match new_event {
                    Removed { id } | Unused { id } => asset_changes.remove(id),
                    Added { id } | Modified { id } | FullyLoaded { id } => {
                        asset_changes.insert(*id, ticks.this_run());
                    }
                };
            }
        }

        messages.write_batch(assets.queued_events.drain(..));
    }

    /// A run condition for [`asset_events`]. The system will not run if there are no events to
    /// flush.
    ///
    /// [`asset_events`]: Self::asset_events
    pub(crate) fn asset_events_condition(assets: Res<Self>) -> bool {
        !assets.queued_events.is_empty()
    }
}

// -----------------------------------------------------------------------------
// ChangeDetection

struct AssetChangeNotifier<'a, A: Asset> {
    changed: bool,
    asset_id: AssetId<A>,
    queued_events: &'a mut Vec<AssetEvent<A>>,
}

/// A change-tracking mutable reference to an asset in [`Assets<A>`].
///
/// Queues an [`AssetEvent::Modified`] on drop if the value was mutably accessed.
/// Use [`into_inner`](Self::into_inner) to consume the guard and always mark the
/// asset as modified, or [`into_inner_untracked`](Self::into_inner_untracked) to
/// get the raw reference without marking it.
pub struct AssetMut<'a, A: Asset> {
    asset: &'a mut A,
    guard: AssetChangeNotifier<'a, A>,
}

impl<'a, A: Asset> Drop for AssetChangeNotifier<'a, A> {
    fn drop(&mut self) {
        if self.changed {
            self.queued_events.push(AssetEvent::Modified { id: self.asset_id });
        }
    }
}

impl<'a, A: Asset> AssetMut<'a, A> {
    pub fn into_inner(mut self) -> &'a mut A {
        self.guard.changed = true;
        self.asset
    }

    pub fn into_inner_untracked(self) -> &'a mut A {
        self.asset
    }
}

impl<'a, A: Asset> Deref for AssetMut<'a, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        self.asset
    }
}

impl<'a, A: Asset> DerefMut for AssetMut<'a, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.changed = true;
        self.asset
    }
}

// -----------------------------------------------------------------------------
// Iterator

pub struct AssetIdIterator<'a, A: Asset> {
    table: core::iter::Enumerate<core::slice::Iter<'a, Option<Entry<A>>>>,
    hash_map: voker_utils::hash::map::Keys<'a, Uuid, A>,
}

impl<'a, A: Asset> Iterator for AssetIdIterator<'a, A> {
    type Item = AssetId<A>;

    fn next(&mut self) -> Option<Self::Item> {
        for (i, entry) in &mut self.table {
            let Some(entry) = entry else {
                continue;
            };
            let Entry { value, generation } = entry;
            if value.is_none() {
                continue;
            }

            return Some(AssetId::Index {
                index: AssetIndex {
                    generation: *generation,
                    index: i as u32,
                },
                marker: PhantomData,
            });
        }

        let key = self.hash_map.next()?;
        Some(AssetId::Uuid { uuid: *key })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower = self.hash_map.len();
        let upper = self.table.len() + lower;
        (lower, Some(lower + upper))
    }
}

pub struct AssetsIterator<'a, A: Asset> {
    table: core::iter::Enumerate<core::slice::Iter<'a, Option<Entry<A>>>>,
    hash_map: voker_utils::hash::map::Iter<'a, Uuid, A>,
}

impl<'a, A: Asset> Iterator for AssetsIterator<'a, A> {
    type Item = (AssetId<A>, &'a A);

    fn next(&mut self) -> Option<Self::Item> {
        for (i, entry) in &mut self.table {
            let Some(entry) = entry else {
                continue;
            };
            let Entry { value, generation } = entry;

            let Some(value) = value else {
                continue;
            };

            let id = AssetId::Index {
                index: AssetIndex {
                    generation: *generation,
                    index: i as u32,
                },
                marker: PhantomData,
            };

            return Some((id, value));
        }

        let (key, value) = self.hash_map.next()?;

        let id = AssetId::Uuid { uuid: *key };
        Some((id, value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower = self.hash_map.len();
        let upper = self.table.len() + lower;
        (lower, Some(lower + upper))
    }
}

pub struct AssetsMutIterator<'a, A: Asset> {
    queued_events: &'a mut Vec<AssetEvent<A>>,
    table: core::iter::Enumerate<core::slice::IterMut<'a, Option<Entry<A>>>>,
    hash_map: voker_utils::hash::map::IterMut<'a, Uuid, A>,
}

impl<'a, A: Asset> Iterator for AssetsMutIterator<'a, A> {
    type Item = (AssetId<A>, &'a mut A);

    fn next(&mut self) -> Option<Self::Item> {
        for (i, entry) in &mut self.table {
            let Some(entry) = entry else {
                continue;
            };
            let Entry { value, generation } = entry;

            let Some(value) = value else {
                continue;
            };

            let id = AssetId::Index {
                index: AssetIndex {
                    generation: *generation,
                    index: i as u32,
                },
                marker: PhantomData,
            };

            self.queued_events.push(AssetEvent::Modified { id });

            return Some((id, value));
        }

        let (key, value) = self.hash_map.next()?;

        let id = AssetId::Uuid { uuid: *key };
        self.queued_events.push(AssetEvent::Modified { id });
        Some((id, value))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower = self.hash_map.len();
        let upper = self.table.len() + lower;
        (lower, Some(lower + upper))
    }
}
