use alloc::string::ToString;
use core::any::TypeId;

use alloc::sync::Arc;

use crate::asset::Asset;
use crate::handle::{ErasedHandle, Handle};
use crate::ident::TypedAssetIndex;
use crate::io::Reader;
use crate::loader::{
    ErasedAssetLoader, ErasedLoadedAsset, ImmediateLoadError, LoadedAsset, LoadedUntypedAsset,
};
use crate::meta::{MetaTransform, Settings};
use crate::path::AssetPath;
use crate::server::{AssetLoadError, MissingAssetLoaderFull, RequestedHandleTypeMismatch};
use crate::utils::{bind_settings_transform, wrap_settings_transform};

use super::LoadContext;

// -----------------------------------------------------------------------------
// Typing & Mode

mod sealed {
    pub trait Typing {}

    pub trait Mode {}
}

pub struct StaticTyped(());
pub struct UnknownTyped(());
pub struct DynamicTyped(TypeId);

pub struct Deferred(());
pub struct Immediate<'builder, 'r>(Option<&'builder mut (dyn Reader + 'r)>);

impl sealed::Typing for StaticTyped {}
impl sealed::Typing for UnknownTyped {}
impl sealed::Typing for DynamicTyped {}

impl sealed::Mode for Deferred {}
impl sealed::Mode for Immediate<'_, '_> {}

// -----------------------------------------------------------------------------
// NestedLoader

pub struct NestedLoader<'ctx, 'builder, T, M> {
    load_context: &'builder mut LoadContext<'ctx>,
    meta_transform: Option<MetaTransform>,
    typing: T,
    mode: M,
}

impl<'ctx, 'builder> NestedLoader<'ctx, 'builder, StaticTyped, Deferred> {
    pub(crate) fn new(load_context: &'builder mut LoadContext<'ctx>) -> Self {
        NestedLoader {
            load_context,
            meta_transform: None,
            typing: StaticTyped(()),
            mode: Deferred(()),
        }
    }
}

// -----------------------------------------------------------------------------
// Basic

impl<'ctx, 'builder, T: sealed::Typing, M: sealed::Mode> NestedLoader<'ctx, 'builder, T, M> {
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

    #[must_use]
    pub fn with_static_type(self) -> NestedLoader<'ctx, 'builder, StaticTyped, M> {
        NestedLoader {
            load_context: self.load_context,
            meta_transform: self.meta_transform,
            typing: StaticTyped(()),
            mode: self.mode,
        }
    }

    #[must_use]
    pub fn with_dynamic_type(
        self,
        asset_type_id: TypeId,
    ) -> NestedLoader<'ctx, 'builder, DynamicTyped, M> {
        NestedLoader {
            load_context: self.load_context,
            meta_transform: self.meta_transform,
            typing: DynamicTyped(asset_type_id),
            mode: self.mode,
        }
    }

    #[must_use]
    pub fn with_unknown_type(self) -> NestedLoader<'ctx, 'builder, UnknownTyped, M> {
        NestedLoader {
            load_context: self.load_context,
            meta_transform: self.meta_transform,
            typing: UnknownTyped(()),
            mode: self.mode,
        }
    }

    #[must_use]
    pub fn deferred(self) -> NestedLoader<'ctx, 'builder, T, Deferred> {
        NestedLoader {
            load_context: self.load_context,
            meta_transform: self.meta_transform,
            typing: self.typing,
            mode: Deferred(()),
        }
    }

    #[must_use]
    pub fn immediate<'c>(self) -> NestedLoader<'ctx, 'builder, T, Immediate<'builder, 'c>> {
        NestedLoader {
            load_context: self.load_context,
            meta_transform: self.meta_transform,
            typing: self.typing,
            mode: Immediate(None),
        }
    }
}

impl<'builder, 'reader, T> NestedLoader<'_, '_, T, Immediate<'builder, 'reader>> {
    #[must_use]
    pub fn with_reader(mut self, reader: &'builder mut (dyn Reader + 'reader)) -> Self {
        self.mode.0 = Some(reader);
        self
    }
}

// -----------------------------------------------------------------------------
// Deferred

impl NestedLoader<'_, '_, StaticTyped, Deferred> {
    pub fn load<'c, A: Asset>(self, path: impl Into<AssetPath<'c>>) -> Handle<A> {
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
                true,
            )
        } else {
            self.load_context
                .asset_server
                .get_or_create_handle::<A>(path, self.meta_transform)
        };

        // `load_with_transform` and `get_or_create_handle` always
        // returns a Strong handle, so we are safe to unwrap.
        let index: TypedAssetIndex = (&handle).try_into().unwrap();
        self.load_context.dependencies.insert(index);

        handle
    }
}

impl NestedLoader<'_, '_, DynamicTyped, Deferred> {
    pub fn load<'p>(self, path: impl Into<AssetPath<'p>>) -> ErasedHandle {
        let path = path.into().into_owned();
        if path.path().as_os_str().is_empty() {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return ErasedHandle::default_for_type(self.typing.0);
        }

        let handle = if self.load_context.should_load_dependencies {
            self.load_context.asset_server.load_erased_asset_impl(
                path,
                self.typing.0,
                None,
                self.meta_transform,
                (),
                false,
            )
        } else {
            self.load_context.asset_server.get_or_create_erased_handle(
                path,
                self.typing.0,
                None,
                self.meta_transform,
            )
        };

        // `load_with_transform` and `get_or_create_handle` always
        // returns a Strong handle, so we are safe to unwrap.
        let index: TypedAssetIndex = (&handle).try_into().unwrap();
        self.load_context.dependencies.insert(index);

        handle
    }
}

impl NestedLoader<'_, '_, UnknownTyped, Deferred> {
    pub fn load<'p>(self, path: impl Into<AssetPath<'p>>) -> Handle<LoadedUntypedAsset> {
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
                false,
            )
        } else {
            self.load_context
                .asset_server
                .get_or_create_handle::<LoadedUntypedAsset>(path, self.meta_transform)
        };

        // `load_unknown_type` and `get_or_create_handle` always
        // returns a Strong handle, so we are safe to unwrap.
        let index: TypedAssetIndex = (&handle).try_into().unwrap();
        self.load_context.dependencies.insert(index);

        handle
    }
}

// -----------------------------------------------------------------------------
// Immediate

impl<'builder, 'reader, T> NestedLoader<'_, '_, T, Immediate<'builder, 'reader>> {
    async fn load_internal(
        self,
        path: &AssetPath<'static>,
        asset_type_id: Option<TypeId>,
    ) -> Result<(Arc<dyn ErasedAssetLoader>, ErasedLoadedAsset), ImmediateLoadError> {
        if path.path().as_os_str().is_empty() {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return Err(ImmediateLoadError::EmptyPath(path.clone()));
        }
        if path.label().is_some() {
            return Err(ImmediateLoadError::RequestedSubasset(path.clone()));
        }

        let load_context = self.load_context;
        let meta_transform = self.meta_transform;
        let reader_opt = self.mode.0;

        let to_load_error = |e: AssetLoadError| ImmediateLoadError::LoadError {
            dependency: path.clone(),
            error: e,
        };

        if let Some(reader) = reader_opt {
            // External reader provided: resolve loader by type / path, use default settings.
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

            // Drop the read guard before the await so the lock is released.
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
            // No external reader: clone the server (cheap Arc clone) so that the
            // reader's borrow does not prevent the subsequent `&mut load_context` call.
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

impl NestedLoader<'_, '_, StaticTyped, Immediate<'_, '_>> {
    pub async fn load<'p, A: Asset>(
        self,
        path: impl Into<AssetPath<'p>>,
    ) -> Result<LoadedAsset<A>, ImmediateLoadError> {
        let path = path.into().into_owned();
        let asset_type_id = Some(TypeId::of::<A>());
        let (loader, asset) = self.load_internal(&path, asset_type_id).await?;

        match asset.downcast::<A>() {
            Ok(typed) => Ok(typed),
            Err(_) => {
                let error = RequestedHandleTypeMismatch {
                    path: path.to_string(),
                    requested: TypeId::of::<A>(),
                    asset_path: loader.asset_type_path(),
                    loader_path: loader.type_path(),
                };
                Err(ImmediateLoadError::LoadError {
                    dependency: path,
                    error: AssetLoadError::from(error),
                })
            }
        }
    }
}

impl NestedLoader<'_, '_, DynamicTyped, Immediate<'_, '_>> {
    pub async fn load<'p>(
        self,
        path: impl Into<AssetPath<'p>>,
    ) -> Result<ErasedLoadedAsset, ImmediateLoadError> {
        let path = path.into().into_owned();
        let asset_type_id = Some(self.typing.0);
        Ok(self.load_internal(&path, asset_type_id).await?.1)
    }
}

impl NestedLoader<'_, '_, UnknownTyped, Immediate<'_, '_>> {
    pub async fn load<'p>(
        self,
        path: impl Into<AssetPath<'p>>,
    ) -> Result<ErasedLoadedAsset, ImmediateLoadError> {
        let path = path.into().into_owned();
        Ok(self.load_internal(&path, None).await?.1)
    }
}
