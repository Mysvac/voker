mod builder;
mod config;
mod error;
mod info;
mod internal;

pub use builder::*;
pub use config::*;
pub use error::*;
pub use info::*;
use internal::*;

// -----------------------------------------------------------------------------
// AssetServer

use alloc::borrow::ToOwned;
use alloc::vec::Vec;
use core::any::TypeId;
use std::path::PathBuf;
use voker_ecs::borrow::ResMut;
use voker_ecs::world::World;
use voker_utils::hash::HashSet;

use alloc::sync::Arc;
use voker_diagnostic::{DiagnosticPath, DiagnosticsStore};
use voker_ecs::borrow::Res;
use voker_ecs::derive::Resource;

use crate::asset::{Asset, VisitAssetDependencies};
use crate::assets::Assets;
use crate::event::ErasedAssetLoadFailedEvent;
use crate::handle::{ErasedHandle, Handle};
use crate::ident::{AssetId, AssetSourceId, ErasedAssetId, TypedAssetIndex};
use crate::io::{
    AssetReaderError, AssetSource, AssetSourceEvent, AssetSources, MissingAssetSource,
};
use crate::loader::{AssetLoader, ErasedAssetLoader};
use crate::loader::{LoadedAsset, LoadedFolder};
use crate::path::AssetPath;

pub const UNTYPED_SOURCE_SUFFIX: &str = "--untyped";

/// Central coordinator for asset loading, caching, and lifecycle tracking.
///
/// `AssetServer` is a cheaply-cloneable handle to a shared `AssetServerData` instance.
/// Add it to your app via [`AssetPlugin`](crate::plugin::AssetPlugin) and access it
/// through `Res<AssetServer>`.
///
/// ## Loading
///
/// ```rust
/// # use voker_asset::{AssetServer, Handle};
/// # use voker_ecs::borrow::Res;
/// # type Image = ();
/// fn startup(server: Res<AssetServer>) {
///     let handle: Handle<Image> = server.load("textures/player.png");
/// }
/// ```
///
/// [`load`](AssetServer::load) is non-blocking: it kicks off an async task and immediately
/// returns a strong [`Handle`].  Read the asset from
/// `Assets<A>` once `AssetServer::is_loaded` returns `true` or an
/// [`AssetEvent::FullyLoaded`](crate::event::AssetEvent::FullyLoaded) is received.
///
/// ## Path deduplication
///
/// If `load` is called for a path that is already in flight or already cached, the server
/// returns a new handle pointing to the same slot — no duplicate IO is performed.
///
/// ## Hot-reload
///
/// Enable the `file_watcher` feature to automatically reload assets when source files
/// change.  [`AssetServer::reload`] also allows manual reloading.
///
/// ## Loader registration
///
/// Loaders must be registered before any asset with a matching extension is loaded:
///
/// ```rust
/// # use voker_asset::{AssetServer, plugin::AppAssetExt};
/// # use voker_app::App;
/// // via App extension
/// // app.register_asset_loader(MyLoader);
/// // or after build
/// // server.register_loader(MyLoader);
/// ```
#[derive(Resource, Clone)]
#[repr(transparent)]
pub struct AssetServer(Arc<AssetServerData>);

impl AssetServer {
    /// Cumulative count of all load tasks started since the server was created.
    pub const STARTED_LOAD_COUNT: DiagnosticPath = DiagnosticPath::new("asset/started_load_count");

    /// Number of load tasks currently in-flight (not yet resolved).
    pub const PENDING_LOAD_COUNT: DiagnosticPath = DiagnosticPath::new("asset/pending_load_count");

    /// System that samples [`STARTED_LOAD_COUNT`](Self::STARTED_LOAD_COUNT) and
    /// [`PENDING_LOAD_COUNT`](Self::PENDING_LOAD_COUNT) into [`DiagnosticsStore`].
    ///
    /// Added automatically by [`AssetDiagnosticsPlugin`](crate::plugin::AssetDiagnosticsPlugin).
    pub fn diagnostic_system(server: Res<AssetServer>, mut store: ResMut<DiagnosticsStore>) {
        use core::sync::atomic::Ordering;

        let started = server.0.stats.started_load_tasks.load(Ordering::Relaxed);
        store.add_measurement(&Self::STARTED_LOAD_COUNT, started as f64);

        let pending = server.read_infos().pending_tasks.len();
        store.add_measurement(&Self::PENDING_LOAD_COUNT, pending as f64);
    }

    pub fn new(
        sources: Arc<AssetSources>,
        server_mode: AssetServerMode,
        meta_check_mode: MetaCheckMode,
        watching_for_changes: bool,
        unapproved_path_mode: UnapprovedPathMode,
    ) -> Self {
        Self::new_impl(
            sources,
            Default::default(),
            server_mode,
            meta_check_mode,
            watching_for_changes,
            unapproved_path_mode,
        )
    }

    #[inline]
    pub fn server_mode(&self) -> AssetServerMode {
        self.0.server_mode
    }

    #[inline]
    pub fn meta_check_mode(&self) -> &MetaCheckMode {
        &self.0.meta_check_mode
    }

    #[inline]
    pub fn unapproved_path_mode(&self) -> &UnapprovedPathMode {
        &self.0.unapproved_path_mode
    }

    #[inline]
    pub fn watching_for_changes(&self) -> bool {
        self.0.watching_for_changes
    }

    #[inline]
    pub fn register_loader<L: AssetLoader>(&self, loader: L) {
        self.register_loader_impl(loader);
    }

    #[inline]
    pub fn register_asset<A: Asset>(&self, assets: &Assets<A>) {
        self.register_asset_impl(assets);
    }

    #[inline]
    pub fn sources(&self) -> &AssetSources {
        &self.0.sources
    }

    #[inline]
    pub fn get_source<'a>(
        &self,
        source: impl Into<AssetSourceId<'a>>,
    ) -> Result<&AssetSource, MissingAssetSource> {
        self.sources().get(source.into())
    }

    pub async fn get_asset_loader_by_path(
        &self,
        loader_path: &str,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoader> {
        let error = || MissingAssetLoader::TypePath(loader_path.to_owned());

        // Separate statements to reduce locked time.
        let opt = self.read_loaders().get_by_path(loader_path);
        opt.ok_or_else(error)?.get().await.map_err(|_| error())
    }

    pub async fn get_asset_loader_by_name(
        &self,
        loader_name: &str,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoader> {
        let error = || MissingAssetLoader::TypeName(loader_name.to_owned());

        // Separate statements to reduce locked time.
        let opt = self.read_loaders().get_by_name(loader_name);
        opt.ok_or_else(error)?.get().await.map_err(|_| error())
    }

    pub async fn get_asset_loader_by_extension(
        &self,
        extension: &str,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoader> {
        let error = || MissingAssetLoader::Extension(alloc::vec![extension.to_owned()]);

        // Separate statements to reduce locked time.
        let opt = self.read_loaders().get_by_extension(extension);
        opt.ok_or_else(error)?.get().await.map_err(|_| error())
    }

    pub async fn get_asset_loader_by_asset_type(
        &self,
        asset_type: TypeId,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoader> {
        let error = || MissingAssetLoader::AssetType(asset_type);

        // Separate statements to reduce locked time.
        let opt = self.read_loaders().get_by_asset_type(asset_type);
        opt.ok_or_else(error)?.get().await.map_err(|_| error())
    }

    pub async fn get_asset_loader_by_asset_path(
        &self,
        asset_path: AssetPath<'_>,
    ) -> Result<Arc<dyn ErasedAssetLoader>, MissingAssetLoader> {
        let error = || MissingAssetLoader::AssetPath(asset_path.clone_owned());

        // Separate statements to reduce locked time.
        let opt = self.read_loaders().get_by_asset_path(&asset_path);
        opt.ok_or_else(error)?.get().await.map_err(|_| error())
    }

    #[inline]
    #[must_use = "the builder do nothing unless you consume it"]
    pub fn load_builder(&self) -> LoadBuilder<'_> {
        LoadBuilder::new(self)
    }

    #[inline]
    #[must_use = "not using returned handle may cause unexpected release of the asset"]
    pub fn load<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Handle<A> {
        self.load_builder().load::<A>(path.into())
    }

    #[inline]
    #[must_use = "not using returned handle may cause unexpected release of the asset"]
    pub fn load_folder<'a>(&self, path: impl Into<AssetPath<'a>>) -> Handle<LoadedFolder> {
        self.load_folder_impl(path.into().into_owned())
    }

    #[inline]
    #[must_use = "not using returned handle may cause unexpected release of the asset"]
    pub fn add<A: Asset>(&self, asset: A) -> Handle<A> {
        self.add_typed_asset_impl(LoadedAsset::with_dependencies(asset))
    }

    #[inline]
    #[must_use = "not using returned handle may cause unexpected release of the asset"]
    pub fn add_async<A: Asset, E: core::error::Error + Send + Sync + 'static>(
        &self,
        asset: impl Future<Output = Result<A, E>> + Send + 'static,
    ) -> Handle<A> {
        self.add_async_impl(asset)
    }

    #[inline]
    pub fn reload<'a>(&self, path: impl Into<AssetPath<'a>>) {
        self.reload_internal(path.into().into_owned(), false);
    }

    pub fn get_erased_handles<'a>(&self, path: impl Into<AssetPath<'a>>) -> Vec<ErasedHandle> {
        let path = path.into();
        self.read_infos().get_handles_by_path(&path)
    }

    pub fn get_one_erased_handle<'a>(
        &self,
        path: impl Into<AssetPath<'a>>,
    ) -> Option<ErasedHandle> {
        let path = path.into();
        self.read_infos().iter_handles_by_path(&path).next()
    }

    pub fn get_erased_ids<'a>(&self, path: impl Into<AssetPath<'a>>) -> Vec<ErasedAssetId> {
        let path = path.into();
        self.read_infos().get_indices_by_path(&path)
    }

    pub fn get_one_erased_id<'a>(&self, path: impl Into<AssetPath<'a>>) -> Option<ErasedAssetId> {
        let path = path.into();
        self.read_infos().iter_indices_by_path(&path).next().map(Into::into)
    }

    pub fn get_erased_handle<'a>(
        &self,
        path: impl Into<AssetPath<'a>>,
        type_id: TypeId,
    ) -> Option<ErasedHandle> {
        let path = path.into();
        self.read_infos().get_handle_by_path_and_type_id(&path, type_id)
    }

    pub fn get_handle<'a, A: Asset>(&self, path: impl Into<AssetPath<'a>>) -> Option<Handle<A>> {
        let path = path.into();
        self.read_infos()
            .get_handle_by_path_and_type_id(&path, TypeId::of::<A>())
            .map(ErasedHandle::typed_debug_checked)
    }

    pub fn get_handle_by_id<A: Asset>(&self, id: AssetId<A>) -> Option<Handle<A>> {
        self.get_erased_handle_by_id(id.erased())
            .map(ErasedHandle::typed_debug_checked)
    }

    pub fn get_erased_handle_by_id(&self, id: ErasedAssetId) -> Option<ErasedHandle> {
        let Ok(index) = id.try_into() else {
            // Always say we don't have Uuid assets.
            return None;
        };
        self.read_infos().get_handle_by_index(index)
    }

    pub fn is_managed(&self, id: impl Into<ErasedAssetId>) -> bool {
        let Ok(index) = id.into().try_into() else {
            // Always say we don't have Uuid assets.
            return false;
        };
        self.read_infos().contains_key(index)
    }

    pub fn get_path(&self, id: impl Into<ErasedAssetId>) -> Option<AssetPath<'_>> {
        let Ok(index) = id.into().try_into() else {
            // Always say we don't have Uuid assets.
            return None;
        };
        let infos = self.read_infos();
        let info = infos.get(index)?;
        Some(info.path.as_ref()?.clone())
    }

    pub fn contains_by_path<'a>(&self, path: impl Into<AssetPath<'a>>) -> bool {
        let path = path.into();
        self.read_infos().contains_by_path(&path)
    }

    pub fn preregister_loader<L: AssetLoader>(&self, extensions: &[&'static str]) {
        self.write_loaders().reserve::<L>(extensions);
    }

    pub async fn wait_for_asset<A: Asset>(
        &self,
        // NOTE: We take a reference to a handle so we know it will outlive the future,
        // which ensures the handle won't be dropped while waiting for the asset.
        handle: &Handle<A>,
    ) -> Result<(), WaitForAssetError> {
        self.wait_for_asset_id(handle.id().erased()).await
    }

    pub async fn wait_for_asset_untyped(
        &self,
        // NOTE: We take a reference to a handle so we know it will outlive the future,
        // which ensures the handle won't be dropped while waiting for the asset.
        handle: &ErasedHandle,
    ) -> Result<(), WaitForAssetError> {
        self.wait_for_asset_id(handle.id()).await
    }

    pub async fn wait_for_asset_id(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> Result<(), WaitForAssetError> {
        let Ok(index) = id.into().try_into() else {
            // Always say we aren't loading Uuid assets.
            return Err(WaitForAssetError::NotLoaded);
        };
        core::future::poll_fn(move |cx| self.wait_for_asset_id_poll_fn(cx, index)).await
    }

    pub async fn write_default_loader_meta_file_for_path(
        &self,
        path: impl Into<AssetPath<'_>>,
    ) -> Result<(), WriteDefaultMetaError> {
        let path = path.into();
        let loader = self.get_asset_loader_by_asset_path(path.clone()).await?;

        let meta = loader.default_meta();
        let serialized_meta = meta.serialize();

        let source = self.get_source(path.source())?;

        let reader = source.reader();
        match reader.read_meta_bytes(path.path()).await {
            Ok(_) => return Err(WriteDefaultMetaError::MetaAlreadyExists),
            Err(AssetReaderError::NotFound(_)) => {
                // The meta file couldn't be found so just fall through.
            }
            Err(AssetReaderError::Io(err)) => {
                return Err(WriteDefaultMetaError::IoErrorFromExistingMetaCheck(
                    Arc::new(err),
                ));
            }
            Err(AssetReaderError::HttpError(err)) => {
                return Err(WriteDefaultMetaError::HttpErrorFromExistingMetaCheck(err));
            }
        }

        let writer = source.writer()?;
        writer.write_meta_bytes(path.path(), &serialized_meta).await?;

        Ok(())
    }

    pub fn get_load_states(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> Option<(LoadState, DependencyLoadState, RecursiveDependencyLoadState)> {
        let Ok(index) = id.into().try_into() else {
            // Always say we don't have Uuid assets.
            return None;
        };

        self.read_infos().get(index).map(|i| {
            (
                i.load_state.clone(),
                i.dep_load_state.clone(),
                i.rec_dep_load_state.clone(),
            )
        })
    }

    pub fn get_load_state(&self, id: impl Into<ErasedAssetId>) -> Option<LoadState> {
        let Ok(index) = id.into().try_into() else {
            // Always say we don't have Uuid assets.
            return None;
        };
        self.read_infos().get(index).map(|i| i.load_state.clone())
    }

    pub fn get_dependency_load_state(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> Option<DependencyLoadState> {
        let Ok(index) = id.into().try_into() else {
            // Always say we don't have Uuid assets.
            return None;
        };
        self.read_infos().get(index).map(|i| i.dep_load_state.clone())
    }

    pub fn get_recursive_dependency_load_state(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> Option<RecursiveDependencyLoadState> {
        let Ok(index) = id.into().try_into() else {
            // Always say we don't have Uuid assets.
            return None;
        };
        self.read_infos().get(index).map(|i| i.rec_dep_load_state.clone())
    }

    pub fn load_state(&self, id: impl Into<ErasedAssetId>) -> LoadState {
        self.get_load_state(id).unwrap_or(LoadState::NotLoaded)
    }

    pub fn dependency_load_state(&self, id: impl Into<ErasedAssetId>) -> DependencyLoadState {
        self.get_dependency_load_state(id)
            .unwrap_or(DependencyLoadState::NotLoaded)
    }

    pub fn recursive_dependency_load_state(
        &self,
        id: impl Into<ErasedAssetId>,
    ) -> RecursiveDependencyLoadState {
        self.get_recursive_dependency_load_state(id)
            .unwrap_or(RecursiveDependencyLoadState::NotLoaded)
    }

    pub fn is_loaded(&self, id: impl Into<ErasedAssetId>) -> bool {
        matches!(self.get_load_state(id), Some(LoadState::Loaded))
    }

    pub fn is_loaded_with_direct_dependencies(&self, id: impl Into<ErasedAssetId>) -> bool {
        matches!(
            self.get_load_states(id),
            Some((LoadState::Loaded, DependencyLoadState::Loaded, _))
        )
    }

    pub fn is_loaded_with_dependencies(&self, id: impl Into<ErasedAssetId>) -> bool {
        matches!(
            self.get_load_states(id),
            Some((
                LoadState::Loaded,
                DependencyLoadState::Loaded,
                RecursiveDependencyLoadState::Loaded
            ))
        )
    }

    pub fn are_dependencies_loaded(&self, value: &impl VisitAssetDependencies) -> bool {
        let infos = self.read_infos();
        let mut loaded = true;
        value.visit_dependencies(&mut |asset_id| {
            let index = match asset_id {
                // Ignore UUID assets - this effectively makes them considered loaded.
                ErasedAssetId::Uuid { .. } => return,
                ErasedAssetId::Index { type_id, index } => TypedAssetIndex::new(index, type_id),
            };

            let Some(info) = infos.get(index) else {
                // If the asset ID is no longer valid, we consider that as not loaded.
                loaded = false;
                return;
            };

            if !info.rec_dep_load_state.is_loaded() {
                loaded = false;
            }
        });
        loaded
    }

    pub fn are_direct_dependencies_loaded(&self, value: &impl VisitAssetDependencies) -> bool {
        let infos = self.read_infos();
        let mut loaded = true;
        value.visit_dependencies(&mut |asset_id| {
            let index = match asset_id {
                // Ignore UUID assets - this effectively makes them considered loaded.
                ErasedAssetId::Uuid { .. } => return,
                ErasedAssetId::Index { type_id, index } => TypedAssetIndex::new(index, type_id),
            };

            let Some(info) = infos.get(index) else {
                // If the asset ID is no longer valid, we consider that as not loaded.
                loaded = false;
                return;
            };

            if !info.dep_load_state.is_loaded() {
                loaded = false;
            }
        });
        loaded
    }
}

pub fn handle_asset_server_events(world: &mut World) {
    world.resource_scope(|world, server: ResMut<AssetServer>| {
        let server = server.as_ref();
        let mut infos = server.write_infos();
        let mut erased_failures = Vec::new();

        for event in server.0.asset_event_receiver.try_iter() {
            match event {
                AssetServerEvent::Loaded {
                    index,
                    loaded_asset,
                } => {
                    infos.process_asset_load(
                        index,
                        loaded_asset,
                        world,
                        &server.0.asset_event_sender,
                    );
                }
                AssetServerEvent::LoadedWithDependencies { index } => {
                    let sender = infos
                        .dependency_loaded_event_sender
                        .get(index.type_id)
                        .expect("Asset event sender should exist");

                    sender(world, index.index);

                    if let Some(info) = infos.get_mut(index) {
                        for waker in core::mem::take(&mut info.waiting_tasks) {
                            waker.wake();
                        }
                    }
                }
                AssetServerEvent::Failed { index, path, error } => {
                    infos.process_asset_fail(index, error.clone());

                    // Send untyped failure event
                    erased_failures.push(ErasedAssetLoadFailedEvent {
                        id: index.into(),
                        path: path.clone(),
                        error: error.clone(),
                    });

                    // Send typed failure event
                    let sender = infos
                        .dependency_failed_event_sender
                        .get(index.type_id)
                        .expect("Asset failed event sender should exist");
                    sender(world, index.index, path, error);
                }
            }
        }

        if !erased_failures.is_empty() {
            world.write_message_batch::<ErasedAssetLoadFailedEvent>(erased_failures);
        }

        if !infos.watching_for_changes {
            return;
        }

        fn queue_ancestors(
            asset_path: &AssetPath,
            infos: &AssetInfos,
            paths_to_reload: &mut HashSet<AssetPath<'static>>,
        ) {
            if let Some(dependents) = infos.loader_dependents.get(asset_path) {
                for dependent in dependents {
                    paths_to_reload.insert(dependent.to_owned());
                    queue_ancestors(dependent, infos, paths_to_reload);
                }
            }
        }

        let mut folders_to_reload = Vec::new();
        let mut reload_parent_folders = |path: &PathBuf, source: &AssetSourceId<'static>| {
            for parent in path.ancestors().skip(1) {
                let parent_asset_path =
                    AssetPath::from(parent.to_path_buf()).with_source(source.clone());
                for folder_handle in infos.iter_handles_by_path(&parent_asset_path) {
                    tracing::info!(
                        "Reloading folder {parent_asset_path} because the content has changed"
                    );
                    folders_to_reload.push((folder_handle, parent_asset_path.clone()));
                }
            }
        };

        let mut paths_to_reload: HashSet<AssetPath<'static>> = HashSet::new();

        let mut reload_path = |path: PathBuf, source: &AssetSourceId<'static>| {
            let path = AssetPath::from(path).with_source(source);
            queue_ancestors(&path, &infos, &mut paths_to_reload);
            paths_to_reload.insert(path);
        };

        let mut handle_event = |source: AssetSourceId<'static>, event: AssetSourceEvent| {
            match event {
                AssetSourceEvent::AddedAsset(path) => {
                    reload_parent_folders(&path, &source);
                    reload_path(path, &source);
                }
                // TODO: if the asset was processed and the processed file was changed, the first modified event
                // should be skipped?
                AssetSourceEvent::ModifiedAsset(path) | AssetSourceEvent::ModifiedMeta(path) => {
                    reload_path(path, &source);
                }
                AssetSourceEvent::RenamedFolder { old, new } => {
                    reload_parent_folders(&old, &source);
                    reload_parent_folders(&new, &source);
                }
                AssetSourceEvent::RemovedAsset(path)
                | AssetSourceEvent::RemovedFolder(path)
                | AssetSourceEvent::AddedFolder(path) => {
                    reload_parent_folders(&path, &source);
                }
                _ => {}
            }
        };

        match server.0.server_mode {
            AssetServerMode::Unprocessed => {
                for source in server.0.sources.iter() {
                    if let Some(receiver) = source.event_receiver() {
                        while let Ok(event) = receiver.try_recv() {
                            handle_event(source.id(), event);
                        }
                    }
                }
            }
            AssetServerMode::Processed => {
                for source in server.0.sources.iter() {
                    if let Some(receiver) = source.processed_event_receiver() {
                        while let Ok(event) = receiver.try_recv() {
                            handle_event(source.id(), event);
                        }
                    }
                }
            }
        }

        voker_task::cfg::single_threaded! {
            ::core::mem::drop(infos);
        }

        for (handle, path) in folders_to_reload {
            // `get_path_handles` only returns Strong variants, so this is safe.
            let index = (&handle).try_into().unwrap();
            server.load_folder_internal(index, path);
        }
        for path in paths_to_reload {
            server.reload_internal(path, true);
        }

        voker_task::cfg::multi_threaded! {
            infos.pending_tasks.retain(|_, load_task| !load_task.is_finished());
        }
    })
}
