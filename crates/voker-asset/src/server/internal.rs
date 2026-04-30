use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::any::TypeId;
use core::panic::AssertUnwindSafe;
use core::task::Poll;
use std::path::Path;

use alloc::sync::Arc;
use crossbeam_channel::{Receiver, Sender};
use futures_lite::{FutureExt, StreamExt};
use voker_ecs::message::MessageQueue;
use voker_ecs::world::World;
use voker_os::sync::{PoisonError, RwLock};
use voker_os::sync::{RwLockReadGuard, RwLockWriteGuard};
use voker_task::IoTaskPool;

use atomicow::CowArc;

use super::UNTYPED_SOURCE_SUFFIX;
use super::{AddAsyncError, LoadState, MissingAssetLoaderFull, WaitForAssetError};
use super::{AssetInfos, AssetLoadError, AssetServer, RequestedHandleTypeMismatch};
use super::{AssetServerMode, MetaCheckMode, UnapprovedPathMode};
use super::{HandleLoadingMode, MissingLabeledAsset, RecursiveDependencyLoadState};
use crate::asset::Asset;
use crate::assets::Assets;
use crate::event::{AssetEvent, AssetLoadFailedEvent};
use crate::handle::{AssetHandleProvider, ErasedHandle, Handle};
use crate::ident::{AssetId, AssetIndex, AssetSourceId, TypedAssetIndex};
use crate::io::{AssetReaderError, AssetSources, ErasedAssetReader, Reader};
use crate::loader::AssetLoaders;
use crate::loader::{
    AssetLoader, AssetLoaderError, AssetLoaderPanic, ErasedAssetLoader, LoadedFolder,
};
use crate::loader::{ErasedLoadedAsset, LoadContext, LoadedAsset, LoadedUntypedAsset};
use crate::meta::{AssetConfigKind, AssetConfigMinimal, DeserializeMetaError};
use crate::meta::{DynamicAssetMeta, MetaTransform, Settings};
use crate::path::AssetPath;

// -----------------------------------------------------------------------------
// Alias

type MetaLoaderReader<'a> = (
    Box<dyn DynamicAssetMeta>,
    Arc<dyn ErasedAssetLoader>,
    Box<dyn Reader + 'a>,
);

// -----------------------------------------------------------------------------
// AssetServerEvent

pub(crate) enum AssetServerEvent {
    Failed {
        index: TypedAssetIndex,
        path: AssetPath<'static>,
        error: AssetLoadError,
    },
    Loaded {
        index: TypedAssetIndex,
        loaded_asset: ErasedLoadedAsset,
    },
    LoadedWithDependencies {
        index: TypedAssetIndex,
    },
}

// -----------------------------------------------------------------------------
// AssetServerData

pub(crate) struct AssetServerData {
    pub infos: RwLock<AssetInfos>,
    pub sources: Arc<AssetSources>,
    pub loaders: Arc<RwLock<AssetLoaders>>,
    pub asset_event_sender: Sender<AssetServerEvent>,
    pub asset_event_receiver: Receiver<AssetServerEvent>,
    pub server_mode: AssetServerMode,
    pub meta_check_mode: MetaCheckMode,
    pub unapproved_path_mode: UnapprovedPathMode,
    pub watching_for_changes: bool, // optional, the type size is unchanged.
}

// -----------------------------------------------------------------------------
// AssetServer Internal API

impl AssetServer {
    fn send_asset_event(&self, event: AssetServerEvent) {
        self.0.asset_event_sender.send(event).unwrap();
    }

    pub(crate) fn new_impl(
        sources: Arc<AssetSources>,
        loaders: Arc<RwLock<AssetLoaders>>,
        server_mode: AssetServerMode,
        meta_check_mode: MetaCheckMode,
        watching_for_changes: bool,
        unapproved_path_mode: UnapprovedPathMode,
    ) -> Self {
        let (asset_event_sender, asset_event_receiver) = crossbeam_channel::unbounded();

        let infos = AssetInfos {
            watching_for_changes,
            ..AssetInfos::default()
        };

        Self(Arc::new(AssetServerData {
            sources,
            server_mode,
            meta_check_mode,
            asset_event_sender,
            asset_event_receiver,
            loaders,
            infos: RwLock::new(infos),
            unapproved_path_mode,
            watching_for_changes,
        }))
    }

    pub(crate) fn read_infos(&self) -> RwLockReadGuard<'_, AssetInfos> {
        self.0.infos.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn write_infos(&self) -> RwLockWriteGuard<'_, AssetInfos> {
        self.0.infos.write().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn read_loaders(&self) -> RwLockReadGuard<'_, AssetLoaders> {
        self.0.loaders.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn write_loaders(&self) -> RwLockWriteGuard<'_, AssetLoaders> {
        self.0.loaders.write().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn register_handle_provider(&self, handle_provider: AssetHandleProvider) {
        self.write_infos()
            .handle_providers
            .insert(handle_provider.type_id, handle_provider);
    }

    pub(crate) fn register_loader_impl<L: AssetLoader>(&self, loader: L) {
        self.write_loaders().push(loader);
    }

    pub(crate) fn register_asset_impl<A: Asset>(&self, assets: &Assets<A>) {
        self.register_handle_provider(assets.handle_provider());

        fn loaded_sender<A: Asset>(world: &mut World, index: AssetIndex) {
            world
                .resource_mut::<MessageQueue<AssetEvent<A>>>()
                .write(AssetEvent::FullyLoaded { id: index.into() });
        }

        fn failed_sender<A: Asset>(
            world: &mut World,
            index: AssetIndex,
            path: AssetPath<'static>,
            error: AssetLoadError,
        ) {
            let id: AssetId<A> = index.into();
            world
                .resource_mut::<MessageQueue<AssetLoadFailedEvent<A>>>()
                .write(AssetLoadFailedEvent { id, path, error });
        }

        let mut infos = self.write_infos();

        infos
            .dependency_loaded_event_sender
            .insert(TypeId::of::<A>(), loaded_sender::<A>);

        infos
            .dependency_failed_event_sender
            .insert(TypeId::of::<A>(), failed_sender::<A>);
    }

    pub(crate) fn get_or_create_handle<'a, A: Asset>(
        &self,
        path: impl Into<AssetPath<'a>>,
        meta_transform: Option<MetaTransform>,
    ) -> Handle<A> {
        self.write_infos()
            .get_or_create_erased_handle(
                path.into().into_owned(),
                TypeId::of::<A>(),
                Some(core::any::type_name::<A>()),
                HandleLoadingMode::NotLoading,
                meta_transform,
            )
            .0
            .typed_unchecked()
    }

    pub(crate) fn get_or_create_erased_handle<'a>(
        &self,
        path: impl Into<AssetPath<'a>>,
        type_id: TypeId,
        debug_name: Option<&str>,
        meta_transform: Option<MetaTransform>,
    ) -> ErasedHandle {
        self.write_infos()
            .get_or_create_erased_handle(
                path.into().into_owned(),
                type_id,
                debug_name,
                HandleLoadingMode::NotLoading,
                meta_transform,
            )
            .0
    }

    #[rustfmt::skip]
    pub(crate) async fn get_meta_loader_and_reader<'a>(
        &'a self,
        asset_path: &'a AssetPath<'_>,
        asset_type_id: Option<TypeId>,
    ) -> Result<MetaLoaderReader<'a>, AssetLoadError> {
        let data = self.0.as_ref();

        let source = data.sources.get(asset_path.source())?;
        let asset_reader = match data.server_mode {
            AssetServerMode::Unprocessed => source.reader(),
            AssetServerMode::Processed => source.processed_reader()?,
        };

        let read_meta = match &data.meta_check_mode {
            MetaCheckMode::Always => true,
            MetaCheckMode::Paths(paths) => paths.contains(asset_path),
            MetaCheckMode::Never => false,
        };

        // ----------------------------------------------------------
        // Not read, use default meta directly
        // ----------------------------------------------------------

        if !read_meta {
            let loader = self.read_loaders().find(
                None, // type_path
                None, // type_name
                asset_type_id,
                None, // extension
                Some(asset_path),
            );

            let error = || -> AssetLoadError {
                MissingAssetLoaderFull {
                    loader_path: None,
                    loader_name: None,
                    asset_type_id,
                    extension: None,
                    asset_path: Some(asset_path.to_string().into_boxed_str()),
                }.into()
            };

            let loader = loader.ok_or_else(error)?.get().await.map_err(|_| error())?;
            let meta = loader.default_meta();
            let reader = asset_reader.read(asset_path.path()).await?;
            return Ok((meta, loader, reader));
        }

        // ----------------------------------------------------------
        // Try read meta
        // ----------------------------------------------------------

        match asset_reader.read_meta(asset_path.path()).await {
            Ok(mut meta_reader) => {
                // ----------------------------------------------------------
                // Not found, use default meta instead
                // ----------------------------------------------------------
                let mut meta_bytes = Vec::new();

                if let Err(e) = meta_reader.read_all_bytes(&mut meta_bytes).await {
                    return Err(AssetReaderError::Io(e).into());
                }

                let error = |e: DeserializeMetaError| {
                    AssetLoadError::DeserializeMetaError {
                        path: asset_path.clone_owned(),
                        error: Box::new(e),
                    }
                };

                // We only need loader information, use `Minimal` to accelerate parsing.
                let minimal = AssetConfigMinimal::from_bytes(&meta_bytes).map_err(error)?;
                let loader_path = match minimal.asset_config {
                    AssetConfigKind::Load { loader } => loader,
                    AssetConfigKind::Process { .. } => {
                        return Err(AssetLoadError::CannotLoadProcessedAsset(asset_path.clone_owned()))
                    }
                    AssetConfigKind::Ignore => {
                        return Err(AssetLoadError::CannotLoadIgnoredAsset(asset_path.clone_owned()))
                    }
                };

                let loader = self.get_asset_loader_by_path(&loader_path).await?;
                let meta = loader.deserialize_meta(&meta_bytes).map_err(error)?;
                let reader = asset_reader.read(asset_path.path()).await?;

                Ok((meta, loader, reader))
            },
            Err(AssetReaderError::NotFound(_)) => {
                // ----------------------------------------------------------
                // Not found, use default meta instead
                // ----------------------------------------------------------
                let loader = self.read_loaders().find(
                    None, // type_path
                    None, // type_name
                    asset_type_id,
                    None, // extension
                    Some(asset_path),
                );

                let error = || -> AssetLoadError {
                    MissingAssetLoaderFull {
                        loader_path: None,
                        loader_name: None,
                        asset_type_id,
                        extension: None,
                        asset_path: Some(asset_path.to_string().into_boxed_str()),
                    }.into()
                };

                let loader = loader.ok_or_else(error)?.get().await.map_err(|_| error())?;
                let meta = loader.default_meta();
                let reader = asset_reader.read(asset_path.path()).await?;
                Ok((meta, loader, reader))
            },
            Err(err) => Err(err.into()),
        }

    }

    /// Performs an async asset load.
    ///
    /// `input_handle` must only be [`Some`] if `should_load` was true when retrieving
    /// `input_handle`. This is an optimization to avoid looking up `should_load` twice, but it
    /// means you _must_ be sure a load is necessary when calling this function with [`Some`].
    ///
    /// Returns the handle of the asset if one was retrieved by this function. Otherwise, may return
    /// [`None`].
    #[rustfmt::skip]
    pub(crate) async fn load_internal<'a>(
        &self,
        input_handle: Option<ErasedHandle>,
        path: AssetPath<'a>,
        force: bool,
        meta_transform: Option<MetaTransform>,
    ) -> Result<Option<ErasedHandle>, AssetLoadError> {
        let asset_type_id = input_handle.as_ref().map(ErasedHandle::type_id);

        let asset_path: AssetPath<'static> = path.into_owned();

        // ----------------------------------------------------------
        // get meta + loader + reader
        // ----------------------------------------------------------

        let mut meta: Box<dyn DynamicAssetMeta>;
        let loader: Arc<dyn ErasedAssetLoader>;
        let mut reader: Box<dyn Reader>;

        match self.get_meta_loader_and_reader(&asset_path, asset_type_id).await {
            Ok(ret) => {
                meta = ret.0;
                loader = ret.1;
                reader = ret.2;
            },
            Err(load_error) => {
                if let Some(handle) = &input_handle {
                    self.send_asset_event(AssetServerEvent::Failed {
                        path: asset_path.clone(),
                        error: load_error.clone(),
                        // The input handle always be strong handle
                        index: handle.try_into().unwrap(),
                    });
                }
                return Err(load_error);
            },
        };

        // ----------------------------------------------------------
        // meta transform
        // ----------------------------------------------------------

        let transform = input_handle.as_ref().and_then(ErasedHandle::meta_transform);
        if let Some(meta_transform) = transform {
            (*meta_transform)(&mut *meta);
        }

        // ----------------------------------------------------------
        // get handle
        // ----------------------------------------------------------

        let asset_id: Option<TypedAssetIndex>; // The asset ID of the asset we are trying to load.
        let fetched_handle: Option<ErasedHandle>; // The handle if one was looked up/created.
        let should_load: bool; // Whether we need to load the asset.

        if let Some(handle) = input_handle {
            // This must have been created with `get_or_create_handle` at some point,
            // which only produces Strong variant handles, so this is safe.
            asset_id = Some((&handle).try_into().unwrap());
            // In this case, we intentionally drop the input handle so we can cancel loading the
            // asset if the handle gets dropped (externally) before it finishes loading.
            fetched_handle = None;
            // The handle was passed in, so the "should_load" check was already done.
            should_load = true;
        } else if asset_path.label().is_none() {
            let (handle, ret_should_load) = self
                .write_infos()
                .get_or_create_erased_handle(
                    asset_path.clone(),
                    loader.asset_type_id(),
                    Some(loader.asset_type_path()),
                    HandleLoadingMode::Request,
                    meta_transform,
                );
            asset_id = Some((&handle).try_into().unwrap());
            fetched_handle = Some(handle);
            should_load = ret_should_load;
        } else {
            let result = self
                .write_infos()
                .try_get_sub_asset_handle(
                    asset_path.clone(),
                    HandleLoadingMode::Request,
                    meta_transform,
                );

            if let Some((handle, ret_should_load)) = result {
                asset_id = Some((&handle).try_into().unwrap());
                fetched_handle = Some(handle);
                should_load = ret_should_load;
            } else {
                // We don't know the expected type since the subasset may have a different type
                // than the "root" asset (which is the type the loader will load).
                asset_id = None;
                fetched_handle = None;
                // If we couldn't find an appropriate handle, then the asset certainly needs to
                // be loaded.
                should_load = true;
            }
        }

        // ----------------------------------------------------------
        // Check TypeId
        // ----------------------------------------------------------

        if let Some(asset_type_id) = asset_id.map(|id| id.type_id)
            && asset_path.label().is_none() && asset_type_id != loader.asset_type_id()
        {
            core::hint::cold_path();

            tracing::error!(
                "Expected {:?}, got {:?}",
                asset_type_id,
                loader.asset_type_id()
            );

            let error = RequestedHandleTypeMismatch {
                path: asset_path.to_string(),
                requested: asset_type_id,
                asset_path: loader.asset_type_path(),
                loader_path: loader.type_path(),
            };
            return Err(error.into());
            // If we are loading a subasset, then the subasset's type almost
            // certainly doesn't match the loader's type - and that's ok.
        }

        // ----------------------------------------------------------
        // !should_load -> return
        // ----------------------------------------------------------

        if !should_load && !force {
            return Ok(fetched_handle);
        }

        // ----------------------------------------------------------
        // load super asset handle, if self is labeld asset
        // ----------------------------------------------------------

        // We don't actually need to use _base_handle,
        // but we do need to keep the handle alive.
        let base_asset_id: TypedAssetIndex;
        let _base_handle: Option<ErasedHandle>;
        let base_path: AssetPath<'static>;

        if asset_path.label().is_some() {
            let pure_path = asset_path.without_label().clone_owned();

            let base_handle = self
                .write_infos()
                .get_or_create_erased_handle(
                    pure_path.clone(),
                    loader.asset_type_id(),
                    Some(loader.asset_type_path()),
                    HandleLoadingMode::Force,
                    None,
                )
                .0;

            base_asset_id = (&base_handle).try_into().unwrap();
            _base_handle = Some(base_handle);
            base_path = pure_path;
        } else {
            // If label is none, get handle must succeed above
            base_asset_id = asset_id.unwrap();
            _base_handle = None;
            base_path = asset_path.clone();
        };

        // ----------------------------------------------------------
        // load asset
        // ----------------------------------------------------------

        let future = self.load_with_loader(
            &base_path,
            meta.loader_settings().expect("meta is set to Load"),
            &*loader,
            &mut *reader,
            true,
            false,
        );

        match future.await {
            Err(err) => {
                // ----------------------------------------------------------
                // load failed
                // ----------------------------------------------------------
                if let Some(asset_id) = asset_id {
                    self.send_asset_event(AssetServerEvent::Failed {
                        index: asset_id,
                        path: asset_path.clone(),
                        error: err.clone(),
                    });
                }
                Err(err)
            },
            Ok(loaded_asset) if let Some(label) = asset_path.label_cow() => {
                // ----------------------------------------------------------
                // load succeed but it's labeled asset
                // ----------------------------------------------------------
                match loaded_asset.label_to_asset_index.get(label.as_ref()) {
                    Some(asset_index) => {
                        let labeled_asset = &loaded_asset.labeled_assets[*asset_index];

                        // If we know the requested type then check it  matches the labeled asset.
                        if let Some(asset_id) = asset_id
                            && asset_id.type_id != labeled_asset.handle.type_id()
                        {
                            let err = RequestedHandleTypeMismatch {
                                path: asset_path.to_string(),
                                requested: asset_id.type_id,
                                asset_path: labeled_asset.asset.value.reflect_type_path(),
                                loader_path: loader.type_path(),
                            };

                            self.send_asset_event(AssetServerEvent::Failed {
                                index: asset_id,
                                path: asset_path.clone(),
                                error: err.clone().into(),
                            });

                            return Err(err.into());
                        }

                        let handle = labeled_asset.handle.clone();
                        self.send_asset_event(AssetServerEvent::Loaded {
                            index: base_asset_id,
                            loaded_asset,
                        });

                        Ok(Some(handle))
                    }
                    None => {
                        let mut all_labels: Vec<String> = loaded_asset
                            .label_to_asset_index
                            .keys()
                            .map(|s| s.as_ref().to_owned())
                            .collect();

                        all_labels.sort_unstable();
                        let error = MissingLabeledAsset {
                            base_path: base_path.to_string(),
                            label: label.to_string(),
                            all_labels,
                        };

                        if let Some(asset_id) = asset_id {
                            self.send_asset_event(AssetServerEvent::Failed {
                                path: asset_path.clone(),
                                index: asset_id,
                                error: error.clone().into(),
                            });
                        }

                        Err(error.into())
                    },
                }
            }
            Ok(loaded_asset) => {
                // ----------------------------------------------------------
                // load succeed but and it's not labeled asset
                // ----------------------------------------------------------
                self.send_asset_event(AssetServerEvent::Loaded {
                    index: base_asset_id,
                    loaded_asset,
                });

                Ok(fetched_handle)
            },
        }
    }

    pub(crate) async fn load_with_loader(
        &self,
        asset_path: &AssetPath<'_>,
        settings: &dyn Settings,
        loader: &dyn ErasedAssetLoader,
        reader: &mut dyn Reader,
        load_dependencies: bool,
        populate_hashes: bool,
    ) -> Result<ErasedLoadedAsset, AssetLoadError> {
        let load_context = LoadContext::new(
            self,
            asset_path.clone_owned(),
            load_dependencies,
            populate_hashes,
        );

        let load = AssertUnwindSafe(loader.load(reader, settings, load_context)).catch_unwind();

        #[cfg(feature = "trace")]
        let load = {
            use tracing::Instrument;

            let span = tracing::info_span!(
                "asset loading",
                loader = loader.type_path(),
                asset = asset_path.to_string(),
            );
            load.instrument(span)
        };

        match load.await {
            Err(_) => {
                core::hint::cold_path();
                let loader_panic = AssetLoaderPanic {
                    path: asset_path.clone_owned(),
                    loader_name: loader.type_path(),
                };
                Err(loader_panic.into())
            }
            Ok(Err(e)) => {
                core::hint::cold_path();
                let loader_error = AssetLoaderError {
                    path: asset_path.clone_owned(),
                    loader_name: loader.type_path(),
                    error: Arc::new(e),
                };
                Err(loader_error.into())
            }
            Ok(Ok(val)) => Ok(val),
        }
    }

    pub(crate) fn spawn_load_task<G: Send + Sync + 'static>(
        &self,
        handle: ErasedHandle,
        path: AssetPath<'static>,
        mut infos: RwLockWriteGuard<AssetInfos>,
        guard: G,
    ) {
        infos.stats.started_load_tasks += 1;

        voker_task::cfg::single_threaded! {
            ::core::mem::drop(infos);
        }

        let input = Some(handle.clone());
        let server = self.clone();

        let task = IoTaskPool::get().spawn(async move {
            if let Err(err) = server.load_internal(input, path, false, None).await {
                tracing::error!("{err}");
            }
            ::core::mem::drop(guard);
        });

        voker_task::cfg::multi_threaded! {
            let mut infos = infos;
            let asset_index: TypedAssetIndex = (&handle).try_into().unwrap();
            infos.pending_tasks.insert(asset_index, task);
        }

        voker_task::cfg::single_threaded! {
            task.detach();
        }
    }

    pub(crate) fn load_typed_asset_impl<A: Asset, G: Send + Sync + 'static>(
        &self,
        path: AssetPath<'static>,
        meta_transform: Option<MetaTransform>,
        guard: G,
        override_unapproved: bool,
    ) -> Handle<A> {
        self.load_erased_asset_impl(
            path,
            TypeId::of::<A>(),
            Some(core::any::type_name::<A>()),
            meta_transform,
            guard,
            override_unapproved,
        )
        .typed_debug_checked::<A>()
    }

    pub(crate) fn load_erased_asset_impl<G: Send + Sync + 'static>(
        &self,
        path: AssetPath<'static>,
        type_id: TypeId,
        debug_name: Option<&str>,
        meta_transform: Option<MetaTransform>,
        guard: G,
        override_unapproved: bool,
    ) -> ErasedHandle {
        if path.path().as_os_str().is_empty() {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return ErasedHandle::default_for_type(type_id);
        }

        if path.is_unapproved() {
            match (&self.0.unapproved_path_mode, override_unapproved) {
                (UnapprovedPathMode::Allow, _) | (UnapprovedPathMode::Deny, true) => {}
                (UnapprovedPathMode::Deny, false) | (UnapprovedPathMode::Forbid, _) => {
                    tracing::error!(
                        "Asset path {path} is unapproved. See UnapprovedPathMode for details."
                    );
                    return ErasedHandle::default_for_type(type_id);
                }
            }
        }

        let mut infos = self.write_infos();

        let (handle, should_load) = infos.get_or_create_erased_handle(
            path.clone(),
            type_id,
            debug_name,
            HandleLoadingMode::Request,
            meta_transform,
        );

        if should_load {
            self.spawn_load_task(handle.clone(), path, infos, guard);
        }

        handle
    }

    pub(crate) fn load_untyped_asset_impl<G: Send + Sync + 'static>(
        &self,
        path: AssetPath<'static>,
        meta_transform: Option<MetaTransform>,
        guard: G,
        override_unapproved: bool,
    ) -> Handle<LoadedUntypedAsset> {
        if path.path().as_os_str().is_empty() {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return Handle::default();
        }

        if path.is_unapproved() {
            match (&self.0.unapproved_path_mode, override_unapproved) {
                (UnapprovedPathMode::Allow, _) | (UnapprovedPathMode::Deny, true) => {}
                (UnapprovedPathMode::Deny, false) | (UnapprovedPathMode::Forbid, _) => {
                    tracing::error!(
                        "Asset path {path} is unapproved. See UnapprovedPathMode for details."
                    );
                    return Handle::default();
                }
            }
        }

        let untyped_source = AssetSourceId::Name(match path.source() {
            AssetSourceId::Default => CowArc::Static(UNTYPED_SOURCE_SUFFIX),
            AssetSourceId::Name(source) => {
                CowArc::Owned(alloc::format!("{source}{UNTYPED_SOURCE_SUFFIX}").into())
            }
        });

        let mut infos = self.write_infos();

        let (handle, should_load) = infos.get_or_create_erased_handle(
            path.clone().with_source(untyped_source),
            TypeId::of::<LoadedUntypedAsset>(),
            Some(core::any::type_name::<LoadedUntypedAsset>()),
            HandleLoadingMode::Request,
            meta_transform,
        );

        let handle = handle.typed_debug_checked::<LoadedUntypedAsset>();

        if !should_load {
            return handle;
        }

        let index: TypedAssetIndex = (&handle).try_into().unwrap();
        infos.stats.started_load_tasks += 1;

        voker_task::cfg::single_threaded! {
            ::core::mem::drop(infos);
        }

        let server = self.clone();
        let task = IoTaskPool::get().spawn(async move {
            let path_clone = path.clone();

            match server.load_internal(None, path, false, None).await {
                Ok(Some(handle)) => {
                    let untyped = LoadedUntypedAsset { handle };
                    let loaded_asset = LoadedAsset::with_dependencies(untyped).erased();
                    let event = AssetServerEvent::Loaded {
                        index,
                        loaded_asset,
                    };
                    server.send_asset_event(event);
                }
                Err(err) => {
                    tracing::error!("{err}");
                    let event = AssetServerEvent::Failed {
                        index,
                        path: path_clone,
                        error: err,
                    };
                    server.send_asset_event(event);
                }
                Ok(None) => {
                    unreachable!(
                        "handle must be returned, since we didn't pass in an input handle"
                    );
                }
            }

            ::core::mem::drop(guard);
        });

        voker_task::cfg::multi_threaded! {
            infos.pending_tasks.insert(index, task);
        }

        voker_task::cfg::single_threaded! {
            task.detach();
        }

        handle
    }

    pub(crate) fn reload_internal(&self, path: AssetPath<'static>, log: bool) {
        let server = self.clone();
        IoTaskPool::get()
            .spawn(async move {
                let mut reloaded = false;

                // First, try to reload the asset for any handles to that path. This will try both
                // root assets and subassets.
                let handles = server.read_infos().get_handles_by_path(&path);

                for handle in handles {
                    // Count each reload as a started load.
                    server.write_infos().stats.started_load_tasks += 1;
                    match server.load_internal(Some(handle), path.clone(), true, None).await {
                        Ok(_) => reloaded = true,
                        Err(err) => tracing::error!("{}", err),
                    }
                }

                // If the above section failed, and there are still living subassets (aka we should
                // reload), then just try doing an untyped load. This helps catch cases where the
                // root asset has been dropped, but all its subassets are still being used (in which
                // case the above section would have tried to find the loader with the root asset's
                // type and loaded it). Hopefully the untyped load will find the right loader and
                // reload all the subassets (though this is not guaranteed).
                if !reloaded && server.read_infos().should_reload(&path) {
                    server.write_infos().stats.started_load_tasks += 1;
                    match server.load_internal(None, path.clone(), true, None).await {
                        Ok(_) => reloaded = true,
                        Err(err) => tracing::error!("{}", err),
                    }
                }

                if log && reloaded {
                    tracing::info!("Reloaded {}", path);
                }
            })
            .detach();
    }

    pub(crate) fn add_typed_asset_impl<A: Asset>(&self, asset: LoadedAsset<A>) -> Handle<A> {
        let erased_loaded_asset: ErasedLoadedAsset = asset.into();
        self.add_erased_asset_impl(None, erased_loaded_asset)
            .typed_debug_checked()
    }

    pub(crate) fn add_erased_asset_impl(
        &self,
        path: Option<AssetPath<'static>>,
        asset: ErasedLoadedAsset,
    ) -> ErasedHandle {
        let loaded_asset = asset;
        let handle = if let Some(path) = path {
            let (handle, _) = self.write_infos().get_or_create_erased_handle(
                path,
                loaded_asset.asset_type_id(),
                Some(loaded_asset.asset_type_path()),
                HandleLoadingMode::NotLoading,
                None,
            );
            handle
        } else {
            self.write_infos().create_loading_erased_handle(
                loaded_asset.asset_type_id(),
                Some(loaded_asset.asset_type_path()),
            )
        };
        self.send_asset_event(AssetServerEvent::Loaded {
            // `get_or_create_handle` always returns Strong handle
            index: (&handle).try_into().unwrap(),
            loaded_asset,
        });
        handle
    }

    pub(crate) fn add_async_impl<A: Asset, E: core::error::Error + Send + Sync + 'static>(
        &self,
        future: impl Future<Output = Result<A, E>> + Send + 'static,
    ) -> Handle<A> {
        let mut infos = self.write_infos();
        let handle = infos
            .create_loading_erased_handle(TypeId::of::<A>(), Some(core::any::type_name::<A>()));

        voker_task::cfg::single_threaded! {
            // drop the lock on `AssetInfos` before spawning a task
            // that may block on it in single-threaded
            ::core::mem::drop(infos);
        }

        // `create_loading_erased_handle` always returns a Strong variant, so this is safe.
        let index = (&handle).try_into().unwrap();

        let event_sender = self.0.asset_event_sender.clone();

        let task = IoTaskPool::get().spawn(async move {
            match future.await {
                Ok(asset) => {
                    let loaded_asset = LoadedAsset::with_dependencies(asset).erased();
                    event_sender
                        .send(AssetServerEvent::Loaded {
                            index,
                            loaded_asset,
                        })
                        .unwrap();
                }
                Err(error) => {
                    let error = AddAsyncError {
                        error: Arc::new(error),
                    };
                    tracing::error!("{error}");
                    event_sender
                        .send(AssetServerEvent::Failed {
                            index,
                            path: Default::default(),
                            error: AssetLoadError::AddAsyncError(error),
                        })
                        .unwrap();
                }
            }
        });

        voker_task::cfg::multi_threaded! {
            infos.pending_tasks.insert(index, task);
        }

        voker_task::cfg::single_threaded! {
            task.detach();
        }

        handle.typed_debug_checked()
    }

    pub(crate) fn load_folder_impl(&self, path: AssetPath<'static>) -> Handle<LoadedFolder> {
        let (handle, should_load) = self.write_infos().get_or_create_erased_handle(
            path.clone(),
            TypeId::of::<LoadedFolder>(),
            Some(core::any::type_name::<LoadedFolder>()),
            HandleLoadingMode::Request,
            None,
        );

        let handle = handle.typed_debug_checked();

        if !should_load {
            return handle;
        }

        // `get_or_create_erased_handle` always returns a Strong variant, so this is safe.
        let index = (&handle).try_into().unwrap();

        self.load_folder_internal(index, path);

        handle
    }

    pub(crate) fn load_folder_internal(&self, index: TypedAssetIndex, path: AssetPath<'static>) {
        async fn load_folder<'a>(
            server: &'a AssetServer,
            source: AssetSourceId<'static>,
            path: &'a Path,
            reader: &'a dyn ErasedAssetReader,
            handles: &'a mut Vec<ErasedHandle>,
        ) -> Result<(), AssetLoadError> {
            let is_dir = reader.is_directory(path).await?;
            if is_dir {
                let mut path_stream = reader.read_directory(path.as_ref()).await?;

                while let Some(child_path) = path_stream.next().await {
                    if reader.is_directory(&child_path).await? {
                        Box::pin(load_folder(
                            server,
                            source.clone(),
                            &child_path,
                            reader,
                            handles,
                        ))
                        .await?;
                    } else {
                        let path = child_path.to_str().expect("Path should be a valid string.");
                        let asset_path = AssetPath::parse(path).with_source(source.clone());
                        match server.load_builder().load_untyped_async(asset_path).await {
                            Ok(handle) => handles.push(handle),
                            // skip assets that cannot be loaded
                            Err(
                                AssetLoadError::MissingAssetLoader(_)
                                | AssetLoadError::MissingAssetLoaderFull(_),
                            ) => {}
                            Err(err) => return Err(err),
                        }
                    }
                }
            }
            Ok(())
        }

        self.write_infos().stats.started_load_tasks += 1;

        let server = self.clone();
        IoTaskPool::get()
            .spawn(async move {
                let Ok(source) = server.get_source(path.source()) else {
                    tracing::error!(
                        "Failed to load {path}. AssetSource {} does not exist",
                        path.source()
                    );
                    return;
                };

                let asset_reader = match server.0.server_mode {
                    AssetServerMode::Unprocessed => source.reader(),
                    AssetServerMode::Processed => match source.processed_reader() {
                        Ok(reader) => reader,
                        Err(_) => {
                            tracing::error!(
                                "Failed to load {path}. AssetSource {} does not have a processed AssetReader",
                                path.source()
                            );
                            return;
                        }
                    },
                };

                let mut handles = Vec::new();
                match load_folder(&server, source.id(), path.path(), asset_reader, &mut handles).await {
                    Ok(_) => {
                        let loaded_asset = LoadedAsset::with_dependencies(LoadedFolder { handles }).erased();
                        server.send_asset_event(AssetServerEvent::Loaded { index, loaded_asset });
                    },
                    Err(err) => {
                        tracing::error!("Failed to load folder. {err}");
                        server.send_asset_event(AssetServerEvent::Failed { index, error: err, path });
                    },
                }
            })
            .detach();
    }

    pub(crate) fn wait_for_asset_id_poll_fn(
        &self,
        cx: &mut core::task::Context<'_>,
        index: TypedAssetIndex,
    ) -> Poll<Result<(), WaitForAssetError>> {
        let infos = self.read_infos();

        let Some(info) = infos.get(index) else {
            return Poll::Ready(Err(WaitForAssetError::NotLoaded));
        };

        match (&info.load_state, &info.rec_dep_load_state) {
            (LoadState::Loaded, RecursiveDependencyLoadState::Loaded) => Poll::Ready(Ok(())),
            // Return an error immediately if the asset is not in the process of loading
            (LoadState::NotLoaded, _) => Poll::Ready(Err(WaitForAssetError::NotLoaded)),
            // If the asset is loading, leave our waker behind
            (LoadState::Loading, _)
            | (_, RecursiveDependencyLoadState::Loading)
            | (LoadState::Loaded, RecursiveDependencyLoadState::NotLoaded) => {
                // Check if our waker is already there
                let has_waker = info.waiting_tasks.iter().any(|waker| waker.will_wake(cx.waker()));

                if has_waker {
                    return Poll::Pending;
                }

                // Must drop read-only guard to acquire write guard
                ::core::mem::drop(infos);
                let mut infos = self.write_infos();

                let Some(info) = infos.get_mut(index) else {
                    return Poll::Ready(Err(WaitForAssetError::NotLoaded));
                };

                // If the load state changed while reacquiring the lock, immediately
                // reawaken the task
                let is_loading = matches!(
                    (&info.load_state, &info.rec_dep_load_state),
                    (LoadState::Loading, _)
                        | (_, RecursiveDependencyLoadState::Loading)
                        | (LoadState::Loaded, RecursiveDependencyLoadState::NotLoaded)
                );

                if !is_loading {
                    cx.waker().wake_by_ref();
                } else {
                    // Leave our waker behind
                    info.waiting_tasks.push(cx.waker().clone());
                }

                Poll::Pending
            }
            (LoadState::Failed(error), _) => {
                Poll::Ready(Err(WaitForAssetError::Failed(error.clone())))
            }
            (_, RecursiveDependencyLoadState::Failed(error)) => {
                Poll::Ready(Err(WaitForAssetError::DependencyFailed(error.clone())))
            }
        }
    }
}
