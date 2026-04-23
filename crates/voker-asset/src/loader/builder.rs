use alloc::boxed::Box;
use core::any::TypeId;
use std::path::Path;

use crate::asset::Asset;
use crate::handle::Handle;
use crate::ident::TypedAssetIndex;
use crate::io::Reader;
use crate::meta::{DynamicAssetMeta, MetaTransform, Settings};
use crate::path::AssetPath;
use crate::utils::meta_transform_settings;

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

impl<'ctx, 'builder, T: sealed::Typing, M: sealed::Mode> NestedLoader<'ctx, 'builder, T, M> {
    fn with_transform(
        mut self,
        transform: impl Fn(&mut dyn DynamicAssetMeta) + Send + Sync + 'static,
    ) -> Self {
        if let Some(prev_transform) = self.meta_transform {
            self.meta_transform = Some(Box::new(move |meta| {
                prev_transform(meta);
                transform(meta);
            }));
        } else {
            self.meta_transform = Some(Box::new(transform));
        }
        self
    }

    #[must_use]
    pub fn with_settings<S: Settings>(
        self,
        settings: impl Fn(&mut S) + Send + Sync + 'static,
    ) -> Self {
        self.with_transform(move |meta| meta_transform_settings(meta, &settings))
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

impl NestedLoader<'_, '_, StaticTyped, Deferred> {
    pub fn load<'c, A: Asset>(self, path: impl Into<AssetPath<'c>>) -> Handle<A> {
        let path = path.into().into_owned();

        if path.path() == Path::new("") {
            tracing::error!("Attempted to load an asset with an empty path \"{path}\"!");
            return Handle::default();
        }

        let handle: Handle<A> = if self.load_context.should_load_dependencies {
            todo!()
        } else {
            todo!()
        };

        let index: TypedAssetIndex = (&handle).try_into().unwrap();

        self.load_context.dependencies.insert(index);

        handle
    }
}
