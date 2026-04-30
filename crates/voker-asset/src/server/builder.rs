use core::any::TypeId;

use alloc::boxed::Box;

use super::AssetServer;
use crate::asset::Asset;
use crate::handle::{ErasedHandle, Handle};
use crate::loader::LoadedUntypedAsset;
use crate::meta::{MetaTransform, Settings};
use crate::path::AssetPath;
use crate::server::{AssetLoadError, UnapprovedPathMode};
use crate::utils::{bind_settings_transform, wrap_settings_transform};

pub struct LoadBuilder<'a> {
    /// The asset server on which the load is invoked.
    asset_server: &'a AssetServer,
    /// A function to modify the meta for an asset loader. In practice, this just mutates the loader
    /// settings of a load.
    meta_transform: Option<MetaTransform>,
    /// Whether unapproved paths are allowed to be loaded.
    override_unapproved: bool,
    /// A "guard" that is held until the load has fully completed.
    guard: Option<Box<dyn Send + Sync + 'static>>,
}

impl<'a> LoadBuilder<'a> {
    #[inline(always)]
    #[must_use = "the builder do nothing unless you consume it"]
    pub(super) fn new(asset_server: &'a AssetServer) -> Self {
        Self {
            asset_server,
            meta_transform: None,
            override_unapproved: false,
            guard: None,
        }
    }

    #[must_use = "the builder do nothing unless you consume it"]
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

    #[must_use = "the builder do nothing unless you consume it"]
    pub fn override_unapproved(mut self) -> Self {
        self.override_unapproved = true;
        self
    }

    #[must_use = "the builder do nothing unless you consume it"]
    pub fn with_guard(mut self, guard: impl Send + Sync + 'static) -> Self {
        if self.guard.is_some() {
            tracing::warn!(
                "Adding a second guard to a LoadBuilder drops the first guard! This is likely a mistake."
            );
        }

        // If guard is already a box, then we might end up double-boxing,
        // which is sad. But this is almost certainly not worth caring about.
        self.guard = Some(Box::new(guard));
        self
    }

    #[must_use]
    fn load_typed_internal(
        self,
        type_id: TypeId,
        debug_name: Option<&str>,
        asset_path: AssetPath<'_>,
    ) -> ErasedHandle {
        self.asset_server.load_erased_asset_impl(
            asset_path.into_owned(),
            type_id,
            debug_name,
            self.meta_transform,
            self.guard,
            self.override_unapproved,
        )
    }

    #[must_use = "not using the returned strong handle may result in the unexpected release of the asset"]
    pub fn load<'b, A: Asset>(self, asset_path: impl Into<AssetPath<'b>>) -> Handle<A> {
        let type_id = TypeId::of::<A>();
        // The type name only used for log, so we can use `core::any::type_name`
        // for better performance, instead of `TypePath::type_path`.
        let debug_name = Some(core::any::type_name::<A>());
        self.load_typed_internal(type_id, debug_name, asset_path.into())
            .typed_debug_checked()
    }

    #[must_use = "not using returned handle may cause unexpected release of the asset"]
    pub fn load_erased<'b>(
        self,
        type_id: TypeId,
        asset_path: impl Into<AssetPath<'b>>,
    ) -> ErasedHandle {
        self.load_typed_internal(type_id, None, asset_path.into())
    }

    #[must_use = "not using returned handle may cause unexpected release of the asset"]
    pub fn load_untyped<'b>(
        self,
        asset_path: impl Into<AssetPath<'b>>,
    ) -> Handle<LoadedUntypedAsset> {
        self.asset_server.load_untyped_asset_impl(
            asset_path.into().into_owned(),
            self.meta_transform,
            self.guard,
            self.override_unapproved,
        )
    }

    // We intentionally don't provide a `load_async` or `load_erased_async`, since these don't
    // provide any value over doing a regular deferred load + `AssetServer::wait_for_asset_id`.
    // `load_untyped_async` on the other hand lets you avoid dealing with the "missing type" of
    // the asset (i.e., dealing with `LoadedUntypedAsset`).

    /// Asynchronously load an asset that you do not know the type of statically.
    ///
    /// If you _do_ know the type of the asset, you should use [`AssetServer::load`].
    ///
    /// If you don't know the type of the asset, but you can't use an async method,
    /// consider using [`LoadBuilder::load_untyped`].
    #[must_use = "not using returned handle may cause unexpected release of the asset"]
    pub async fn load_untyped_async<'b>(
        self,
        asset_path: impl Into<AssetPath<'b>>,
    ) -> Result<ErasedHandle, AssetLoadError> {
        let path: AssetPath = asset_path.into();
        if path.path().as_os_str().is_empty() {
            return Err(AssetLoadError::EmptyPath(path.into_owned()));
        }

        if path.is_unapproved() {
            match (
                self.asset_server.unapproved_path_mode(),
                self.override_unapproved,
            ) {
                (UnapprovedPathMode::Allow, _) | (UnapprovedPathMode::Deny, true) => {}
                (UnapprovedPathMode::Deny, false) | (UnapprovedPathMode::Forbid, _) => {
                    return Err(AssetLoadError::UnapprovedPath(path.into_owned()));
                }
            }
        }

        self.asset_server.add_started_load_tasks();

        // Hold the guard for the duration of the async load so callers can
        // use it to synchronise on completion (e.g. a barrier or channel).
        let _guard = self.guard;

        let result = self
            .asset_server
            .load_internal(None, path, false, self.meta_transform)
            .await;

        // handle must be returned, since we didn't pass in an input handle
        result.map(|h| h.unwrap())
    }
}
