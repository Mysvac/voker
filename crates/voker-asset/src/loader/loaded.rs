use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::{Any, TypeId};

use atomicow::CowArc;
use voker_ecs::world::World;
use voker_utils::hash::{HashMap, HashSet};

use crate::asset::Asset;
use crate::handle::ErasedHandle;
use crate::ident::{AssetIndex, ErasedAssetId, TypedAssetIndex};
use crate::meta::AssetHash;
use crate::path::AssetPath;

// -----------------------------------------------------------------------------
// AssetContainer

pub(crate) trait AssetContainer: Any + Send + Sync + 'static {
    fn asset_insert(self: Box<Self>, id: AssetIndex, world: &mut World);
    fn asset_type_name(&self) -> &'static str;
}

impl<A: Asset> AssetContainer for A {
    fn asset_insert(self: Box<Self>, index: AssetIndex, world: &mut World) {
        todo!()
    }

    #[inline]
    fn asset_type_name(&self) -> &'static str {
        core::any::type_name::<A>()
    }
}

// -----------------------------------------------------------------------------
// LoadedAsset

pub(crate) struct LabeledAsset {
    pub(crate) asset: ErasedLoadedAsset,
    pub(crate) handle: ErasedHandle,
}

pub struct ErasedLoadedAsset {
    pub(crate) value: Box<dyn AssetContainer>,
    pub(crate) dependencies: HashSet<TypedAssetIndex>,
    pub(crate) loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,
    pub(crate) labeled_assets: Vec<LabeledAsset>,
    pub(crate) label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    pub(crate) asset_id_to_asset_index: HashMap<ErasedAssetId, usize>,
}

pub struct LoadedAsset<A: Asset> {
    pub(crate) value: A,
    pub(crate) dependencies: HashSet<TypedAssetIndex>,
    pub(crate) loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,
    pub(crate) labeled_assets: Vec<LabeledAsset>,
    pub(crate) label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    pub(crate) asset_id_to_asset_index: HashMap<ErasedAssetId, usize>,
}

// -----------------------------------------------------------------------------
// LoadedAsset Implementation

impl<A: Asset> LoadedAsset<A> {
    pub fn with_dependencies(value: A) -> Self {
        let mut dependencies = HashSet::<TypedAssetIndex>::new();

        value.visit_dependencies(&mut |id| {
            if let Ok(asset_index) = id.try_into() {
                dependencies.insert(asset_index);
            }
        });

        LoadedAsset {
            value,
            dependencies,
            loader_dependencies: HashMap::new(),
            labeled_assets: Vec::new(),
            label_to_asset_index: HashMap::new(),
            asset_id_to_asset_index: HashMap::new(),
        }
    }

    pub fn take(self) -> A {
        self.value
    }

    pub fn get(&self) -> &A {
        &self.value
    }

    pub fn iter_labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.label_to_asset_index.keys().map(|s| &**s)
    }

    pub fn get_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedLoadedAsset> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        let labeled = &self.labeled_assets[*index];
        Some(&labeled.asset)
    }

    pub fn get_labeled_by_id(&self, id: impl Into<ErasedAssetId>) -> Option<&ErasedLoadedAsset> {
        let index = self.asset_id_to_asset_index.get(&id.into())?;
        let labeled = &self.labeled_assets[*index];
        Some(&labeled.asset)
    }
}

impl<A: Asset> From<A> for LoadedAsset<A> {
    #[inline]
    fn from(asset: A) -> Self {
        LoadedAsset::with_dependencies(asset)
    }
}

impl<A: Asset> From<LoadedAsset<A>> for ErasedLoadedAsset {
    #[inline]
    fn from(asset: LoadedAsset<A>) -> Self {
        ErasedLoadedAsset {
            value: Box::new(asset.value),
            dependencies: asset.dependencies,
            loader_dependencies: asset.loader_dependencies,
            labeled_assets: asset.labeled_assets,
            label_to_asset_index: asset.label_to_asset_index,
            asset_id_to_asset_index: asset.asset_id_to_asset_index,
        }
    }
}

// -----------------------------------------------------------------------------
// LoadedAsset Implementation

impl ErasedLoadedAsset {
    pub fn take<A: Asset>(self) -> Option<A> {
        <Box<dyn Any>>::downcast::<A>(self.value).map(|a| *a).ok()
    }

    pub fn get<A: Asset>(&self) -> Option<&A> {
        <dyn Any>::downcast_ref::<A>(&self.value)
    }

    pub fn asset_type_id(&self) -> TypeId {
        self.value.as_ref().type_id()
    }

    pub fn asset_type_name(&self) -> &'static str {
        self.value.asset_type_name()
    }

    pub fn get_labeled(&self, label: impl AsRef<str>) -> Option<&ErasedLoadedAsset> {
        let index = self.label_to_asset_index.get(label.as_ref())?;
        let labeled = &self.labeled_assets[*index];
        Some(&labeled.asset)
    }

    pub fn get_labeled_by_id(&self, id: impl Into<ErasedAssetId>) -> Option<&ErasedLoadedAsset> {
        let index = self.asset_id_to_asset_index.get(&id.into())?;
        let labeled = &self.labeled_assets[*index];
        Some(&labeled.asset)
    }

    pub fn iter_labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.label_to_asset_index.keys().map(|s| &**s)
    }

    #[inline]
    pub fn downcast<A: Asset>(self) -> Result<LoadedAsset<A>, ErasedLoadedAsset> {
        if self.value.as_ref().type_id() == TypeId::of::<A>() {
            #[expect(unsafe_code, reason = "already checked")]
            let value: Box<A> =
                unsafe { <Box<dyn Any>>::downcast::<A>(self.value).unwrap_unchecked() };
            Ok(LoadedAsset {
                value: *value,
                dependencies: self.dependencies,
                loader_dependencies: self.loader_dependencies,
                labeled_assets: self.labeled_assets,
                label_to_asset_index: self.label_to_asset_index,
                asset_id_to_asset_index: self.asset_id_to_asset_index,
            })
        } else {
            Err(self)
        }
    }
}
