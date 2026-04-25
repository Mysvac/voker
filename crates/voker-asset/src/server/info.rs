use alloc::boxed::Box;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::any::TypeId;
use core::task::Waker;

use crossbeam_channel::Sender;
use voker_ecs::derive::Component;
use voker_ecs::world::World;
use voker_task::Task;
use voker_utils::extra::TypeIdMap;
use voker_utils::hash::{HashMap, HashSet};

use super::AssetServerEvent;
use crate::handle::{AssetHandleProvider, ErasedHandle, StrongHandle};
use crate::ident::{AssetIndex, ErasedAssetId, TypedAssetIndex};
use crate::loader::ErasedLoadedAsset;
use crate::meta::{AssetHash, MetaTransform};
use crate::path::AssetPath;
use crate::server::{AssetLoadError, MissingHandleProvider};

// -----------------------------------------------------------------------------
// LoadState

/// The load state of an asset.
#[derive(Component, Clone, Debug)]
pub enum LoadState {
    NotLoaded,
    Loading,
    Loaded,
    Failed(Arc<AssetLoadError>),
}

impl LoadState {
    /// Returns `true` if this instance is [`LoadState::NotLoaded`]
    pub const fn is_not_loaded(&self) -> bool {
        matches!(self, Self::NotLoaded)
    }

    /// Returns `true` if this instance is [`LoadState::Loading`]
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Returns `true` if this instance is [`LoadState::Loaded`]
    pub const fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    /// Returns `true` if this instance is [`LoadState::Failed`]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

// -----------------------------------------------------------------------------
// DependencyLoadState

/// The load state of an asset's dependencies.
#[derive(Component, Clone, Debug)]
pub enum DependencyLoadState {
    NotLoaded,
    Loading,
    Loaded,
    Failed(Arc<AssetLoadError>),
}

impl DependencyLoadState {
    /// Returns `true` if this instance is [`DependencyLoadState::NotLoaded`]
    pub const fn is_not_loaded(&self) -> bool {
        matches!(self, Self::NotLoaded)
    }

    /// Returns `true` if this instance is [`DependencyLoadState::Loading`]
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Returns `true` if this instance is [`DependencyLoadState::Loaded`]
    pub const fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    /// Returns `true` if this instance is [`DependencyLoadState::Failed`]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

// -----------------------------------------------------------------------------
// RecursiveDependencyLoadState

/// The recursive load state of an asset's dependencies.
#[derive(Component, Clone, Debug)]
pub enum RecursiveDependencyLoadState {
    NotLoaded,
    Loading,
    Loaded,
    Failed(Arc<AssetLoadError>),
}

impl RecursiveDependencyLoadState {
    /// Returns `true` if this instance is [`DependencyLoadState::NotLoaded`]
    pub const fn is_not_loaded(&self) -> bool {
        matches!(self, Self::NotLoaded)
    }

    /// Returns `true` if this instance is [`RecursiveDependencyLoadState::Loading`]
    pub const fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    /// Returns `true` if this instance is [`RecursiveDependencyLoadState::Loaded`]
    pub const fn is_loaded(&self) -> bool {
        matches!(self, Self::Loaded)
    }

    /// Returns `true` if this instance is [`RecursiveDependencyLoadState::Failed`]
    pub const fn is_failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }
}

// -----------------------------------------------------------------------------
// LoadState

/// Determines how a handle should be initialized
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum HandleLoadingMode {
    /// The handle is for an asset that isn't loading/loaded yet.
    NotLoading,
    /// The handle is for an asset that is being _requested_ to load (if it isn't already loading)
    Request,
    /// The handle is for an asset that is being forced to load (even if it has already loaded)
    Force,
}

// -----------------------------------------------------------------------------
// AssetInfo

#[derive(Debug)]
pub(crate) struct AssetInfo {
    pub(crate) weak_handle: Weak<StrongHandle>,
    pub(crate) path: Option<AssetPath<'static>>,
    pub(crate) load_state: LoadState,
    pub(crate) dep_load_state: DependencyLoadState,
    pub(crate) rec_dep_load_state: RecursiveDependencyLoadState,
    /// 正向索引，当前资产直接依赖的项目
    pub(crate) loading_dependencies: HashSet<TypedAssetIndex>,
    /// 正向索引，当前资产直接依赖的项目
    pub(crate) failed_dependencies: HashSet<TypedAssetIndex>,
    /// 正向索引，所有当前资产依赖的项目
    pub(crate) loading_rec_dependencies: HashSet<TypedAssetIndex>,
    /// 正向索引，所有当前资产依赖的项目
    pub(crate) failed_rec_dependencies: HashSet<TypedAssetIndex>,
    /// 反向索引，直接依赖当前资产的项目
    pub(crate) dependents_waiting_on_load: HashSet<TypedAssetIndex>,
    /// 反向索引，所有依赖当前资产的项目
    pub(crate) dependents_waiting_on_recursive_dep_load: HashSet<TypedAssetIndex>,
    /// The asset paths required to load this asset.
    ///
    /// Hashes will only be set for processed assets.
    /// This is set using the value from [`LoadedAsset`].
    /// This will only be populated if [`AssetInfos::watching_for_changes`]
    /// is set to `true` to save memory.
    ///
    /// [`LoadedAsset`]: crate::loader::LoadedAsset
    pub(crate) loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,
    /// The number of handle drops to skip for this asset.
    pub(crate) handle_drops_to_skip: usize,
    /// List of tasks waiting for this asset to complete loading
    pub(crate) waiting_tasks: Vec<Waker>,
}

impl AssetInfo {
    fn new(weak_handle: Weak<StrongHandle>, path: Option<AssetPath<'static>>) -> Self {
        Self {
            weak_handle,
            path,
            load_state: LoadState::NotLoaded,
            dep_load_state: DependencyLoadState::NotLoaded,
            rec_dep_load_state: RecursiveDependencyLoadState::NotLoaded,
            loading_dependencies: HashSet::new(),
            failed_dependencies: HashSet::new(),
            loading_rec_dependencies: HashSet::new(),
            failed_rec_dependencies: HashSet::new(),
            loader_dependencies: HashMap::new(),
            dependents_waiting_on_load: HashSet::new(),
            dependents_waiting_on_recursive_dep_load: HashSet::new(),
            handle_drops_to_skip: 0,
            waiting_tasks: Vec::new(),
        }
    }
}

// -----------------------------------------------------------------------------
// AssetServerStats

/// Tracks statistics of the asset server.
#[derive(Default, Clone, PartialEq, Eq)]
pub(crate) struct AssetServerStats {
    /// The number of load tasks that have been started.
    pub(crate) started_load_tasks: usize,
}

// -----------------------------------------------------------------------------
// AssetInfos

type DepLoadedEventSender = fn(&mut World, AssetIndex);
type DepFailedEventSender = fn(&mut World, AssetIndex, AssetPath<'static>, AssetLoadError);

#[derive(Default)]
pub(crate) struct AssetInfos {
    pub infos: HashMap<TypedAssetIndex, AssetInfo>,
    pub path_to_index: HashMap<AssetPath<'static>, TypeIdMap<AssetIndex>>,
    pub handle_providers: TypeIdMap<AssetHandleProvider>,
    pub watching_for_changes: bool,
    // 反向索引，Key 被哪些 Value 依赖。
    pub loader_dependents: HashMap<AssetPath<'static>, HashSet<AssetPath<'static>>>,
    pub living_labeled_assets: HashMap<AssetPath<'static>, HashSet<Box<str>>>,
    pub dependency_loaded_event_sender: TypeIdMap<DepLoadedEventSender>,
    pub dependency_failed_event_sender: TypeIdMap<DepFailedEventSender>,
    pub pending_tasks: HashMap<TypedAssetIndex, Task<()>>,
    pub stats: AssetServerStats,
}

impl core::fmt::Debug for AssetInfos {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AssetInfos")
            .field("path_to_index", &self.path_to_index)
            .field("infos", &self.infos)
            .finish()
    }
}

// -----------------------------------------------------------------------------
// AssetInfos

#[inline]
pub(crate) fn unwrap_with_context<T>(
    result: Result<T, MissingHandleProvider>,
    type_id: TypeId,
    debug_name: Option<&str>,
) -> Option<T> {
    #[cold]
    #[inline(never)]
    fn handle_error(type_id: TypeId, debug_name: Option<&str>) -> ! {
        match debug_name {
            Some(debug_name) => {
                panic!(
                    "Cannot allocate an AssetHandle of type '{debug_name}' because the asset type has not been initialized. \
                    Make sure you have called `app.init_asset::<{debug_name}>()`"
                );
            }
            None => {
                panic!(
                    "Cannot allocate an AssetHandle of type '{type_id:?}' because the asset type has not been initialized. \
                    Make sure you have called `app.init_asset::<(actual asset type)>()`"
                )
            }
        }
    }

    match result {
        Ok(value) => Some(value),
        Err(_) => handle_error(type_id, debug_name),
    }
}

// -----------------------------------------------------------------------------
// create_handle_internal & get_or_create_handle_internal

impl AssetInfos {
    /// 根据 TypeId 创建资产，资产路径可为空（因此可动态创建）。
    ///
    /// - 分配新 `AssetInfo` 并插入 `AssetInfos`。
    /// - 此函数**不**插入 path_to_index 索引。
    /// - 如果 loading == true, info 的三个状态将设为 loading。
    /// - 如果 watching_for_changes = true 且存在资产路径，`living_labeled_assets` 会插入当前值。
    fn create_handle_internal(
        infos: &mut HashMap<TypedAssetIndex, AssetInfo>,
        handle_providers: &TypeIdMap<AssetHandleProvider>,
        living_labeled_assets: &mut HashMap<AssetPath<'static>, HashSet<Box<str>>>,
        watching_for_changes: bool,
        loading: bool,
        type_id: TypeId,
        path: Option<AssetPath<'static>>,
        meta_transform: Option<MetaTransform>,
    ) -> Result<ErasedHandle, MissingHandleProvider> {
        let provider = handle_providers.get(type_id).ok_or(MissingHandleProvider(type_id))?;

        if watching_for_changes && let Some(path) = &path {
            let mut without_label = path.clone();
            if let Some(label) = without_label.take_label() {
                let entry = living_labeled_assets.entry(without_label);
                entry.or_default().insert(label.as_ref().into());
            }
        }

        let handle = provider.alloc_handle(true, path.clone(), meta_transform);

        let mut info = AssetInfo::new(Arc::downgrade(&handle), path);

        if loading {
            info.load_state = LoadState::Loading;
            info.dep_load_state = DependencyLoadState::Loading;
            info.rec_dep_load_state = RecursiveDependencyLoadState::Loading;
        }

        infos.insert(TypedAssetIndex::new(handle.index, handle.type_id), info);

        Ok(ErasedHandle::Strong(handle))
    }

    /// 根据 path 和 type_id 获取资产句柄
    ///
    /// 如果资产不存在，会创建 AssetInfo 并插入（通过 `create_handle_internal`）。
    ///
    /// 如果已存在，直接获取句柄（句柄失效时重分配）。
    fn get_or_create_handle_internal(
        &mut self,
        path: AssetPath<'static>,
        type_id: TypeId,
        loading_mode: HandleLoadingMode,
        meta_transform: Option<MetaTransform>,
    ) -> Result<(ErasedHandle, bool), MissingHandleProvider> {
        use voker_utils::extra::TypeIdMapEntry;

        let key = path.clone();
        let handles = self.path_to_index.entry(key).or_default();

        match handles.entry(type_id) {
            TypeIdMapEntry::Occupied(entry) => {
                let index = *entry.get();

                // if there is a path_to_id entry, info always exists
                let info = self.infos.get_mut(&TypedAssetIndex::new(index, type_id)).unwrap();

                let should_load = match loading_mode {
                    HandleLoadingMode::Force => true,
                    HandleLoadingMode::Request => {
                        matches!(info.load_state, LoadState::NotLoaded | LoadState::Failed(_))
                    }
                    _ => false,
                };

                if should_load {
                    info.load_state = LoadState::Loading;
                    info.dep_load_state = DependencyLoadState::Loading;
                    info.rec_dep_load_state = RecursiveDependencyLoadState::Loading;
                }

                if let Some(strong_handle) = info.weak_handle.upgrade() {
                    // If we can upgrade the handle, there is at least one live handle right now,
                    // The asset load has already kicked off (and maybe completed), so we can just
                    // return a strong handle
                    Ok((ErasedHandle::Strong(strong_handle), should_load))
                } else {
                    // Asset meta exists, but all live handles were dropped. This means the `track_assets` system
                    // hasn't been run yet to remove the current asset
                    // (note that this is guaranteed to be transactional with the `track_assets` system
                    // because it locks the AssetInfos collection)

                    let provider = self
                        .handle_providers
                        .get(type_id)
                        .ok_or(MissingHandleProvider(type_id))?;

                    // We created a new strong handle, need to skip one drop info request.
                    info.handle_drops_to_skip += 1;

                    // We must create a new strong handle for the existing id and ensure that the drop of the old
                    // strong handle doesn't remove the asset from the Assets collection
                    let handle = provider.build_handle(index, true, Some(path), meta_transform);
                    info.weak_handle = Arc::downgrade(&handle);
                    Ok((ErasedHandle::Strong(handle), should_load))
                }
            }
            TypeIdMapEntry::Vacant(entry) => {
                let should_load = match loading_mode {
                    HandleLoadingMode::NotLoading => false,
                    HandleLoadingMode::Request | HandleLoadingMode::Force => true,
                };
                let handle = Self::create_handle_internal(
                    &mut self.infos,
                    &self.handle_providers,
                    &mut self.living_labeled_assets,
                    self.watching_for_changes,
                    should_load,
                    type_id,
                    Some(path),
                    meta_transform,
                )?;
                let index = match &handle {
                    ErasedHandle::Strong(handle) => handle.index,
                    // `create_handle_internal` always returns Strong variant.
                    ErasedHandle::Uuid { .. } => unreachable!(),
                };
                entry.insert(index);
                Ok((handle, should_load))
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Basic

impl AssetInfos {
    pub fn create_loading_erased_handle(
        &mut self,
        type_id: TypeId,
        debug_name: Option<&str>,
    ) -> ErasedHandle {
        let result = Self::create_handle_internal(
            &mut self.infos,
            &self.handle_providers,
            &mut self.living_labeled_assets,
            self.watching_for_changes,
            true,
            type_id,
            None,
            None,
        );

        unwrap_with_context(result, type_id, debug_name).unwrap()
    }

    pub fn get_or_create_erased_handle(
        &mut self,
        path: AssetPath<'static>,
        type_id: TypeId,
        debug_name: Option<&str>,
        loading_mode: HandleLoadingMode,
        meta_transform: Option<MetaTransform>,
    ) -> (ErasedHandle, bool) {
        let result =
            self.get_or_create_handle_internal(path, type_id, loading_mode, meta_transform);

        // it is ok to unwrap because TypeId was specified above
        unwrap_with_context(result, type_id, debug_name).unwrap()
    }

    pub fn try_get_sub_asset_handle(
        &mut self,
        path: AssetPath<'static>,
        loading_mode: HandleLoadingMode,
        meta_transform: Option<MetaTransform>,
    ) -> Option<(ErasedHandle, bool)> {
        let handles = self.path_to_index.entry(path.clone()).or_default();

        let type_id = if handles.len() == 1 {
            // If a TypeId is not provided, we may be able
            // to infer it if only a single entry exists
            handles.types().next().unwrap()
        } else {
            return None;
        };

        Some(self.get_or_create_erased_handle(path, type_id, None, loading_mode, meta_transform))
    }

    pub fn contains_key(&self, index: TypedAssetIndex) -> bool {
        self.infos.contains_key(&index)
    }

    pub fn get(&self, index: TypedAssetIndex) -> Option<&AssetInfo> {
        self.infos.get(&index)
    }

    pub fn get_mut(&mut self, index: TypedAssetIndex) -> Option<&mut AssetInfo> {
        self.infos.get_mut(&index)
    }

    pub fn iter_indices_by_path<'a>(
        &'a self,
        path: &'a AssetPath<'_>,
    ) -> impl ExactSizeIterator<Item = TypedAssetIndex> + 'a {
        /// Concrete type to allow returning an `impl Iterator` even if `self.path_to_id.get(&path)` is `None`
        enum TypedAssetIndexIter<T> {
            None,
            Some(T),
        }

        impl<T> Iterator for TypedAssetIndexIter<T>
        where
            T: Iterator<Item = TypedAssetIndex>,
        {
            type Item = TypedAssetIndex;

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    TypedAssetIndexIter::None => None,
                    TypedAssetIndexIter::Some(iter) => iter.next(),
                }
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                match self {
                    TypedAssetIndexIter::None => (0, Some(0)),
                    TypedAssetIndexIter::Some(iter) => iter.size_hint(),
                }
            }
        }

        impl<T> ExactSizeIterator for TypedAssetIndexIter<T>
        where
            T: ExactSizeIterator<Item = TypedAssetIndex>,
        {
            #[inline]
            fn len(&self) -> usize {
                match self {
                    TypedAssetIndexIter::None => 0,
                    TypedAssetIndexIter::Some(iter) => iter.len(),
                }
            }
        }

        if let Some(mapper) = self.path_to_index.get(path) {
            let iter = mapper
                .iter()
                .map(|(type_id, index)| TypedAssetIndex::new(*index, type_id));
            TypedAssetIndexIter::Some(iter)
        } else {
            TypedAssetIndexIter::None
        }
    }

    pub fn iter_handles_by_path<'a>(
        &'a self,
        path: &'a AssetPath<'_>,
    ) -> impl Iterator<Item = ErasedHandle> + 'a {
        self.iter_indices_by_path(path)
            .filter_map(|id| self.get_handle_by_index(id))
    }

    pub fn get_handles_by_path<'a>(&'a self, path: &'a AssetPath<'_>) -> Vec<ErasedHandle> {
        if let Some(mapper) = self.path_to_index.get(path) {
            let mut buffer = Vec::with_capacity(mapper.len());
            for (type_id, index) in mapper.iter() {
                let index = TypedAssetIndex::new(*index, type_id);
                if let Some(handle) = self.get_handle_by_index(index) {
                    buffer.push(handle);
                }
            }
            return buffer;
        }
        Vec::new()
    }

    pub fn get_indices_by_path<'a>(&'a self, path: &'a AssetPath<'_>) -> Vec<ErasedAssetId> {
        if let Some(mapper) = self.path_to_index.get(path) {
            let mut buffer: Vec<ErasedAssetId> = Vec::with_capacity(mapper.len());
            for (type_id, index) in mapper.iter() {
                let index = TypedAssetIndex::new(*index, type_id);
                if self.get_handle_by_index(index).is_some() {
                    buffer.push(index.into());
                }
            }
            return buffer;
        }
        Vec::new()
    }

    pub fn contains_by_path<'a>(&'a self, path: &'a AssetPath<'_>) -> bool {
        if let Some(mapper) = self.path_to_index.get(path) {
            for (type_id, index) in mapper.iter() {
                let index = TypedAssetIndex::new(*index, type_id);
                if self.contains_by_index(index) {
                    return true;
                } // as same as `iter + map + any`, but with same format as above.
            }
        }
        false
    }

    pub fn get_handle_by_index(&self, index: TypedAssetIndex) -> Option<ErasedHandle> {
        let info = self.infos.get(&index)?;
        let strong_handle = info.weak_handle.upgrade()?;
        Some(ErasedHandle::Strong(strong_handle))
    }

    pub fn contains_by_index(&self, index: TypedAssetIndex) -> bool {
        if let Some(info) = self.infos.get(&index) {
            info.weak_handle.strong_count() != 0
        } else {
            false
        }
    }

    pub fn get_handle_by_path_and_type_id(
        &self,
        path: &AssetPath<'_>,
        type_id: TypeId,
    ) -> Option<ErasedHandle> {
        let index = *self.path_to_index.get(path)?.get(type_id)?;
        self.get_handle_by_index(TypedAssetIndex::new(index, type_id))
    }

    pub fn is_path_alive<'a>(&self, path: impl Into<AssetPath<'a>>) -> bool {
        self.iter_indices_by_path(&path.into())
            .filter_map(|id| self.infos.get(&id))
            .any(|info| info.weak_handle.strong_count() > 0)
    }

    pub fn should_reload(&self, path: &AssetPath) -> bool {
        if self.is_path_alive(path) {
            return true;
        }

        if let Some(living) = self.living_labeled_assets.get(path) {
            !living.is_empty()
        } else {
            false
        }
    }
}

// -----------------------------------------------------------------------------
// handle drop

impl AssetInfos {
    fn remove_dependents_and_labels(
        info: &AssetInfo,
        loader_dependents: &mut HashMap<AssetPath<'static>, HashSet<AssetPath<'static>>>,
        path: &AssetPath<'static>,
        living_labeled_assets: &mut HashMap<AssetPath<'static>, HashSet<Box<str>>>,
    ) {
        use voker_utils::hash::map::Entry;

        for loader_dependency in info.loader_dependencies.keys() {
            if let Some(dependents) = loader_dependents.get_mut(loader_dependency) {
                dependents.remove(path);
            }
        }

        let Some(label) = path.label() else {
            return;
        };

        let mut without_label = path.clone_owned();
        without_label.remove_label();

        let Entry::Occupied(mut entry) = living_labeled_assets.entry(without_label) else {
            return;
        };

        entry.get_mut().remove(label);
        if entry.get().is_empty() {
            entry.remove();
        }
    }

    fn process_handle_drop_internal(
        infos: &mut HashMap<TypedAssetIndex, AssetInfo>,
        path_to_index: &mut HashMap<AssetPath<'static>, TypeIdMap<AssetIndex>>,
        loader_dependents: &mut HashMap<AssetPath<'static>, HashSet<AssetPath<'static>>>,
        living_labeled_assets: &mut HashMap<AssetPath<'static>, HashSet<Box<str>>>,
        pending_tasks: &mut HashMap<TypedAssetIndex, Task<()>>,
        watching_for_changes: bool,
        index: TypedAssetIndex,
    ) -> bool {
        use voker_utils::hash::map::Entry;

        let Entry::Occupied(mut entry) = infos.entry(index) else {
            // Either the asset was already dropped, it doesn't exist, or it isn't managed by the asset server
            // None of these cases should result in a removal from the Assets collection
            return false;
        };

        if entry.get_mut().handle_drops_to_skip > 0 {
            entry.get_mut().handle_drops_to_skip -= 1;
            return false;
        }

        pending_tasks.remove(&index);

        let type_id = entry.key().type_id;

        let info = entry.remove();

        let Some(path) = &info.path else {
            return true;
        };

        if watching_for_changes {
            Self::remove_dependents_and_labels(
                &info,
                loader_dependents,
                path,
                living_labeled_assets,
            );
        }

        if let Some(map) = path_to_index.get_mut(path) {
            map.remove(type_id);

            if map.is_empty() {
                path_to_index.remove(path);
            }
        };

        true
    }

    /// Returns `true` if the asset should be removed from the collection.
    pub fn process_handle_drop(&mut self, index: TypedAssetIndex) -> bool {
        Self::process_handle_drop_internal(
            &mut self.infos,
            &mut self.path_to_index,
            &mut self.loader_dependents,
            &mut self.living_labeled_assets,
            &mut self.pending_tasks,
            self.watching_for_changes,
            index,
        )
    }

    pub fn process_handle_drop_events(&mut self) {
        for provider in self.handle_providers.values() {
            while let Ok(drop_event) = provider.drop_receiver.try_recv() {
                let id = drop_event.index;
                if drop_event.asset_server_managed {
                    Self::process_handle_drop_internal(
                        &mut self.infos,
                        &mut self.path_to_index,
                        &mut self.loader_dependents,
                        &mut self.living_labeled_assets,
                        &mut self.pending_tasks,
                        self.watching_for_changes,
                        id,
                    );
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// propagate

impl AssetInfos {
    /// Recursively propagates loaded state up the dependency tree.
    fn propagate_loaded_state(
        &mut self,
        loaded_id: TypedAssetIndex,
        waiting_id: TypedAssetIndex,
        sender: &Sender<AssetServerEvent>,
    ) {
        if let Some(info) = self.infos.get_mut(&waiting_id) {
            info.loading_rec_dependencies.remove(&loaded_id);

            if info.loading_rec_dependencies.is_empty() && info.failed_rec_dependencies.is_empty() {
                info.rec_dep_load_state = RecursiveDependencyLoadState::Loaded;
                info.loading_rec_dependencies = HashSet::new(); // dealloc memory

                if info.load_state.is_loaded() {
                    let event = AssetServerEvent::LoadedWithDependencies { index: waiting_id };
                    sender.send(event).unwrap();
                }

                for dep_id in core::mem::take(&mut info.dependents_waiting_on_recursive_dep_load) {
                    self.propagate_loaded_state(waiting_id, dep_id, sender);
                }
            }
        };
    }

    /// Recursively propagates failed state up the dependency tree
    fn propagate_failed_state(
        &mut self,
        failed_id: TypedAssetIndex,
        waiting_id: TypedAssetIndex,
        error: &Arc<AssetLoadError>,
    ) {
        if let Some(info) = self.infos.get_mut(&waiting_id) {
            info.loading_rec_dependencies.remove(&failed_id);
            info.failed_rec_dependencies.insert(failed_id);
            info.rec_dep_load_state = RecursiveDependencyLoadState::Failed(error.clone());

            for dep_id in core::mem::take(&mut info.dependents_waiting_on_recursive_dep_load) {
                self.propagate_failed_state(waiting_id, dep_id, error);
            }
        };
    }
}

impl AssetInfos {
    pub fn process_asset_fail(&mut self, failed_index: TypedAssetIndex, error: AssetLoadError) {
        let Some(info) = self.infos.get_mut(&failed_index) else {
            // already be removed
            return;
        };

        let error = Arc::new(error);

        let (waiting_on_load, waiting_on_rec_load) = {
            info.load_state = LoadState::Failed(error.clone());
            info.dep_load_state = DependencyLoadState::Failed(error.clone());
            info.rec_dep_load_state = RecursiveDependencyLoadState::Failed(error.clone());

            for waker in core::mem::take(&mut info.waiting_tasks) {
                waker.wake();
            }

            (
                core::mem::take(&mut info.dependents_waiting_on_load),
                core::mem::take(&mut info.dependents_waiting_on_recursive_dep_load),
            )
        };

        for waiting_id in waiting_on_load {
            if let Some(info) = self.infos.get_mut(&waiting_id) {
                info.loading_dependencies.remove(&failed_index);
                info.failed_dependencies.insert(failed_index);
                // don't overwrite DependencyLoadState if already failed to preserve first error
                if !info.dep_load_state.is_failed() {
                    info.dep_load_state = DependencyLoadState::Failed(error.clone());
                }
            }
        }

        for waiting_id in waiting_on_rec_load {
            self.propagate_failed_state(failed_index, waiting_id, &error);
        }
    }

    pub fn process_asset_load(
        &mut self,
        loaded_index: TypedAssetIndex,
        loaded_asset: ErasedLoadedAsset,
        world: &mut World,
        sender: &Sender<AssetServerEvent>,
    ) {
        for asset in loaded_asset.labeled_assets {
            let ErasedHandle::Strong(handle) = &asset.handle else {
                unreachable!("Labeled assets are always strong handles");
            };
            let label_index = TypedAssetIndex {
                index: handle.index,
                type_id: handle.type_id,
            };
            self.process_asset_load(label_index, asset.asset, world, sender);
        }

        if !self.infos.contains_key(&loaded_index) {
            return;
        }

        loaded_asset.value.apply_asset(loaded_index.index, world);

        let mut loading_deps: HashSet<TypedAssetIndex> = loaded_asset.dependencies;
        let mut failed_deps: HashSet<TypedAssetIndex> = HashSet::new();
        let mut dep_error: Option<Arc<AssetLoadError>> = None;

        let mut loading_rec_deps: HashSet<TypedAssetIndex> = loading_deps.clone();
        let mut failed_rec_deps: HashSet<TypedAssetIndex> = HashSet::new();
        let mut rec_dep_error: Option<Arc<AssetLoadError>> = None;

        loading_deps.retain(|dep_id| {
            if let Some(dep_info) = self.infos.get_mut(dep_id) {
                match &dep_info.rec_dep_load_state {
                    RecursiveDependencyLoadState::Loading
                    | RecursiveDependencyLoadState::NotLoaded => {
                        // If dependency is loading, wait for it.
                        dep_info.dependents_waiting_on_recursive_dep_load.insert(loaded_index);
                    }
                    RecursiveDependencyLoadState::Loaded => {
                        // If dependency is loaded, reduce our count by one
                        loading_rec_deps.remove(dep_id);
                    }
                    RecursiveDependencyLoadState::Failed(error) => {
                        if rec_dep_error.is_none() {
                            rec_dep_error = Some(error.clone());
                        }
                        failed_rec_deps.insert(*dep_id);
                        loading_rec_deps.remove(dep_id);
                    }
                }
                match &dep_info.load_state {
                    LoadState::NotLoaded | LoadState::Loading => {
                        // If dependency is loading, wait for it.
                        dep_info.dependents_waiting_on_load.insert(loaded_index);
                        true
                    }
                    LoadState::Loaded => {
                        // If dependency is loaded, reduce our count by one
                        false
                    }
                    LoadState::Failed(error) => {
                        if dep_error.is_none() {
                            dep_error = Some(error.clone());
                        }
                        failed_deps.insert(*dep_id);
                        false
                    }
                }
            } else {
                tracing::warn!(
                    "Dependency {} from asset {} is unknown. This asset's dependency load status \
                    will not switch to 'Loaded' until the unknown dependency is loaded.",
                    dep_id,
                    loaded_index
                );
                true
            }
        });

        let dep_load_state = match (loading_deps.len(), failed_deps.len()) {
            (0, 0) => DependencyLoadState::Loaded,
            (_loading, 0) => DependencyLoadState::Loading,
            (_loading, _failed) => DependencyLoadState::Failed(dep_error.unwrap()),
        };

        let rec_dep_load_state = match (loading_rec_deps.len(), failed_rec_deps.len()) {
            (0, 0) => {
                let event = AssetServerEvent::LoadedWithDependencies {
                    index: loaded_index,
                };
                sender.send(event).unwrap();
                RecursiveDependencyLoadState::Loaded
            }
            (_loading, 0) => RecursiveDependencyLoadState::Loading,
            (_loading, _failed) => RecursiveDependencyLoadState::Failed(rec_dep_error.unwrap()),
        };

        let (waiting_on_load, waiting_on_rec_load) = {
            // Asset info should always exist at this point
            let info = self.infos.get_mut(&loaded_index).unwrap();

            // if watching for changes, track reverse loader dependencies for hot reloading
            if self.watching_for_changes {
                if let Some(asset_path) = &info.path {
                    for loader_dependency in loaded_asset.loader_dependencies.keys() {
                        self.loader_dependents
                            .entry(loader_dependency.clone())
                            .or_default()
                            .insert(asset_path.clone());
                    }
                }

                info.loader_dependencies = loaded_asset.loader_dependencies;
            }

            info.loading_dependencies = loading_deps;
            info.failed_dependencies = failed_deps;
            info.loading_rec_dependencies = loading_rec_deps;
            info.failed_rec_dependencies = failed_rec_deps;
            info.load_state = LoadState::Loaded;
            info.dep_load_state = dep_load_state;
            info.rec_dep_load_state = rec_dep_load_state.clone();

            let rec_load_finished =
                rec_dep_load_state.is_failed() || rec_dep_load_state.is_loaded();
            let dependents_waiting_on_rec_load = if rec_load_finished {
                Some(core::mem::take(
                    &mut info.dependents_waiting_on_recursive_dep_load,
                ))
            } else {
                None
            };

            (
                core::mem::take(&mut info.dependents_waiting_on_load),
                dependents_waiting_on_rec_load,
            )
        };

        for id in waiting_on_load {
            if let Some(info) = self.infos.get_mut(&id) {
                info.loading_dependencies.remove(&loaded_index);
                if info.loading_dependencies.is_empty() && !info.dep_load_state.is_failed() {
                    // send dependencies loaded event
                    info.dep_load_state = DependencyLoadState::Loaded;
                    info.loading_dependencies = HashSet::new(); // dealloc memory
                }
            }
        }

        if let Some(waiting_on_rec_load) = waiting_on_rec_load {
            match &rec_dep_load_state {
                RecursiveDependencyLoadState::Loaded => {
                    for dep_id in waiting_on_rec_load {
                        Self::propagate_loaded_state(self, loaded_index, dep_id, sender);
                    }
                }
                RecursiveDependencyLoadState::Failed(error) => {
                    for dep_id in waiting_on_rec_load {
                        Self::propagate_failed_state(self, loaded_index, dep_id, error);
                    }
                }
                RecursiveDependencyLoadState::Loading | RecursiveDependencyLoadState::NotLoaded => {
                    unreachable!("Should not be `Loading` or `NotLoaded`, checked above.")
                }
            }
        }
    }
}
