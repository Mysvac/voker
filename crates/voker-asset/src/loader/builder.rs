use alloc::string::ToString;
use alloc::sync::Arc;
use core::any::TypeId;

use crate::asset::Asset;
use crate::handle::{ErasedHandle, Handle};
use crate::ident::TypedAssetIndex;
use crate::io::Reader;
use crate::loader::{ErasedAssetLoader, ErasedLoadedAsset, LoadDirectError};
use crate::loader::{LoadedAsset, LoadedUntypedAsset};
use crate::meta::{MetaTransform, Settings};
use crate::path::AssetPath;
use crate::server::{AssetLoadError, MissingAssetLoaderFull, RequestedHandleTypeMismatch};
use crate::utils::{bind_settings_transform, wrap_settings_transform};

use super::LoadContext;

// -----------------------------------------------------------------------------
// NestedLoadBuilder

/// A builder for loading nested assets inside a [`LoadContext`].
///
/// Obtained from [`LoadContext::load_builder`].  Configure the load with
/// [`with_settings`](NestedLoadBuilder::with_settings) and
/// [`override_unapproved`](NestedLoadBuilder::override_unapproved), then
/// consume the builder by calling one of the terminal methods.
///
/// # Deferred loads (non-async)
///
/// These methods immediately return a handle and schedule the IO work:
///
/// - [`load`](NestedLoadBuilder::load) — static type `A`
/// - [`load_erased`](NestedLoadBuilder::load_erased) — dynamic `TypeId`
/// - [`load_untyped`](NestedLoadBuilder::load_untyped) — type inferred from extension / meta
///
/// # Immediate loads (async)
///
/// These methods run the loader inline and return the loaded data:
///
/// - [`load_value`](NestedLoadBuilder::load_value) — static type `A`
/// - [`load_erased_value`](NestedLoadBuilder::load_erased_value) — dynamic `TypeId`
/// - [`load_untyped_value`](NestedLoadBuilder::load_untyped_value) — unknown type
/// - `*_from_reader` variants read from a caller-supplied [`Reader`] instead of opening the file
pub struct NestedLoadBuilder<'ctx, 'builder> {
    load_context: &'builder mut LoadContext<'ctx>,
    meta_transform: Option<MetaTransform>,
    override_unapproved: bool,
}

impl<'ctx, 'builder> NestedLoadBuilder<'ctx, 'builder> {
    pub(crate) fn new(load_context: &'builder mut LoadContext<'ctx>) -> Self {
        NestedLoadBuilder {
            load_context,
            meta_transform: None,
            override_unapproved: false,
        }
    }

    /// Override the asset's loader settings.
    ///
    /// Chaining multiple calls composes the transformations in call order.
    #[must_use]
    pub fn with_settings<S: Settings>(
        mut self,
        settings: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        if let Some(prev_transform) = self.meta_transform.take() {
            self.meta_transform = Some(bind_settings_transform(prev_transform, settings));
        } else {
            self.meta_transform = Some(wrap_settings_transform(settings));
        }
        self
    }

    /// Allow loading from unapproved paths (paths that escape the source root) even if
    /// [`UnapprovedPathMode::Deny`](crate::server::UnapprovedPathMode::Deny) is active.
    #[must_use]
    pub fn override_unapproved(mut self) -> Self {
        self.override_unapproved = true;
        self
    }
}

// -----------------------------------------------------------------------------
// Deferred

impl<'ctx, 'builder> NestedLoadBuilder<'ctx, 'builder> {
    /// Deferred-load `path` as asset type `A`, returning a handle immediately.
    ///
    /// The actual IO is performed asynchronously; the returned handle will resolve
    /// once the load completes.
    pub fn load<'p, A: Asset>(self, path: impl Into<AssetPath<'p>>) -> Handle<A> {
        let path = path.into().into_owned();
        if path.path().as_os_str().is_empty() {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return Handle::default();
        }

        let handle: Handle<A> = if self.load_context.should_load_dependencies {
            self.load_context.asset_server.load_typed_asset_impl::<A, ()>(
                path,
                self.meta_transform,
                (),
                self.override_unapproved,
            )
        } else {
            self.load_context
                .asset_server
                .get_or_create_handle::<A>(path, self.meta_transform)
        };

        // `load_typed_asset_impl` / `get_or_create_handle` always return a Strong handle.
        let index: TypedAssetIndex = (&handle).try_into().unwrap();
        self.load_context.dependencies.insert(index);
        handle
    }

    /// Deferred-load `path` with a runtime-known `type_id`, returning an erased handle.
    pub fn load_erased<'p>(self, type_id: TypeId, path: impl Into<AssetPath<'p>>) -> ErasedHandle {
        let path = path.into().into_owned();
        if path.path().as_os_str().is_empty() {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return ErasedHandle::default_for_type(type_id);
        }

        let handle = if self.load_context.should_load_dependencies {
            self.load_context.asset_server.load_erased_asset_impl(
                path,
                type_id,
                None,
                self.meta_transform,
                (),
                self.override_unapproved,
            )
        } else {
            self.load_context.asset_server.get_or_create_erased_handle(
                path,
                type_id,
                None,
                self.meta_transform,
            )
        };

        let index: TypedAssetIndex = (&handle).try_into().unwrap();
        self.load_context.dependencies.insert(index);
        handle
    }

    /// Deferred-load `path` with an unknown type (inferred from extension or `.meta`),
    /// returning a `Handle<LoadedUntypedAsset>`.
    pub fn load_untyped<'p>(self, path: impl Into<AssetPath<'p>>) -> Handle<LoadedUntypedAsset> {
        let path = path.into().into_owned();
        if path.path().as_os_str().is_empty() {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return Handle::default();
        }

        let handle = if self.load_context.should_load_dependencies {
            self.load_context.asset_server.load_untyped_asset_impl(
                path,
                self.meta_transform,
                (),
                self.override_unapproved,
            )
        } else {
            self.load_context
                .asset_server
                .get_or_create_handle::<LoadedUntypedAsset>(path, self.meta_transform)
        };

        let index: TypedAssetIndex = (&handle).try_into().unwrap();
        self.load_context.dependencies.insert(index);
        handle
    }
}

// -----------------------------------------------------------------------------
// Immediate

impl<'ctx, 'builder> NestedLoadBuilder<'ctx, 'builder> {
    /// Immediately load `path` as asset type `A`, returning the loaded data.
    pub async fn load_value<'p, A: Asset>(
        self,
        path: impl Into<AssetPath<'p>>,
    ) -> Result<LoadedAsset<A>, LoadDirectError> {
        let path = path.into().into_owned();
        self.load_typed_internal::<A>(path, None).await
    }

    /// Immediately load `path` with a runtime-known `type_id`, returning the loaded data.
    pub async fn load_erased_value<'p>(
        self,
        type_id: TypeId,
        path: impl Into<AssetPath<'p>>,
    ) -> Result<ErasedLoadedAsset, LoadDirectError> {
        let path = path.into().into_owned();
        self.load_internal(&path, Some(type_id), None).await.map(|(_, a)| a)
    }

    /// Immediately load `path` with an unknown type, returning the loaded data.
    pub async fn load_untyped_value<'p>(
        self,
        path: impl Into<AssetPath<'p>>,
    ) -> Result<ErasedLoadedAsset, LoadDirectError> {
        let path = path.into().into_owned();
        self.load_internal(&path, None, None).await.map(|(_, a)| a)
    }

    /// Immediately load from `reader` as asset type `A`.
    ///
    /// `path` is used for sub-asset handles and relative-path resolution;
    /// no file is opened.
    pub async fn load_value_from_reader<'p, A: Asset>(
        self,
        path: impl Into<AssetPath<'p>>,
        reader: &'builder mut dyn Reader,
    ) -> Result<LoadedAsset<A>, LoadDirectError> {
        let path = path.into().into_owned();
        self.load_typed_internal::<A>(path, Some(reader)).await
    }

    /// Immediately load from `reader` with a runtime-known `type_id`.
    pub async fn load_erased_value_from_reader<'p>(
        self,
        type_id: TypeId,
        path: impl Into<AssetPath<'p>>,
        reader: &'builder mut dyn Reader,
    ) -> Result<ErasedLoadedAsset, LoadDirectError> {
        let path = path.into().into_owned();
        self.load_internal(&path, Some(type_id), Some(reader))
            .await
            .map(|(_, a)| a)
    }

    /// Immediately load from `reader` with an unknown type.
    pub async fn load_untyped_value_from_reader<'p>(
        self,
        path: impl Into<AssetPath<'p>>,
        reader: &'builder mut dyn Reader,
    ) -> Result<ErasedLoadedAsset, LoadDirectError> {
        let path = path.into().into_owned();
        self.load_internal(&path, None, Some(reader)).await.map(|(_, a)| a)
    }

    /// Typed wrapper that downcasts the erased result into `LoadedAsset<A>`.
    async fn load_typed_internal<A: Asset>(
        self,
        path: AssetPath<'static>,
        reader: Option<&'builder mut dyn Reader>,
    ) -> Result<LoadedAsset<A>, LoadDirectError> {
        let (loader, asset) = self.load_internal(&path, Some(TypeId::of::<A>()), reader).await?;
        match asset.downcast::<A>() {
            Ok(typed) => Ok(typed),
            Err(_) => {
                let error = RequestedHandleTypeMismatch {
                    path: path.to_string(),
                    requested: TypeId::of::<A>(),
                    asset_path: loader.asset_type_path(),
                    loader_path: loader.type_path(),
                };
                Err(LoadDirectError::LoadError {
                    dependency: path,
                    error: AssetLoadError::from(error),
                })
            }
        }
    }

    /// Core async load: resolves the loader, applies `meta_transform`, and runs the load.
    ///
    /// If `reader` is `Some`, it is used directly and no file is opened.
    /// Otherwise the asset is read from the source registered with the asset server.
    async fn load_internal(
        self,
        path: &AssetPath<'static>,
        asset_type_id: Option<TypeId>,
        reader: Option<&'builder mut dyn Reader>,
    ) -> Result<(Arc<dyn ErasedAssetLoader>, ErasedLoadedAsset), LoadDirectError> {
        if path.path().as_os_str().is_empty() {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return Err(LoadDirectError::EmptyPath(path.clone()));
        }
        if path.label().is_some() {
            return Err(LoadDirectError::RequestedSubasset(path.clone()));
        }

        let load_context = self.load_context;
        load_context.asset_server.add_started_load_tasks();
        let meta_transform = self.meta_transform;

        let to_load_error = |e: AssetLoadError| LoadDirectError::LoadError {
            dependency: path.clone(),
            error: e,
        };

        if let Some(reader) = reader {
            let missing_loader = || -> AssetLoadError {
                MissingAssetLoaderFull {
                    loader_path: None,
                    loader_name: None,
                    asset_type_id,
                    extension: None,
                    asset_path: Some(path.to_string().into_boxed_str()),
                }
                .into()
            };

            // Release the read-lock before the await.
            let maybe = load_context.asset_server.read_loaders().find(
                None,
                None,
                asset_type_id,
                None,
                Some(path),
            );

            let loader = maybe
                .ok_or_else(|| to_load_error(missing_loader()))?
                .get()
                .await
                .map_err(|_| to_load_error(missing_loader()))?;

            let mut meta = loader.default_meta();
            if let Some(transform) = meta_transform {
                transform(&mut *meta);
            }
            let settings = meta.loader_settings().expect("default meta is always Load");

            let loaded_asset = load_context
                .load_direct_internal(path.clone(), settings, &*loader, reader, None)
                .await?;

            Ok((loader, loaded_asset))
        } else {
            // Clone the server (cheap Arc clone) to release the borrow on `load_context`
            // before the subsequent `&mut load_context` call.
            let server = load_context.asset_server.clone();
            let (mut meta, loader, mut reader) = server
                .get_meta_loader_and_reader(path, asset_type_id)
                .await
                .map_err(to_load_error)?;

            if let Some(transform) = meta_transform {
                transform(&mut *meta);
            }
            let settings = meta.loader_settings().expect("meta is always Load");

            let loaded_asset = load_context
                .load_direct_internal(path.clone(), settings, &*loader, &mut *reader, None)
                .await?;

            Ok((loader, loaded_asset))
        }
    }
}
