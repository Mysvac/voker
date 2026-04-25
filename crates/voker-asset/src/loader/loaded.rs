use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::{Any, TypeId};

use atomicow::CowArc;
use voker_ecs::world::World;
use voker_reflect::info::{DynamicTypePath, TypePath};
use voker_utils::hash::{HashMap, HashSet};

use crate::asset::{Asset, VisitAssetDependencies};
use crate::assets::Assets;
use crate::handle::ErasedHandle;
use crate::ident::{AssetIndex, ErasedAssetId, TypedAssetIndex};
use crate::meta::AssetHash;
use crate::path::AssetPath;

// -----------------------------------------------------------------------------
// AssetContainer

pub(crate) trait AssetContainer: DynamicTypePath + Any + Send + Sync + 'static {
    fn apply_asset(self: Box<Self>, id: AssetIndex, world: &mut World);
}

impl<A: Asset> AssetContainer for A {
    fn apply_asset(self: Box<Self>, index: AssetIndex, world: &mut World) {
        world
            .resource_mut::<Assets<A>>()
            .insert(index, *self)
            .expect("the AssetIndex is still valid");
    }
}

// --------------------------------------------------------------
// LoadedFolder

#[derive(TypePath)]
#[type_path = "voker_asset::loader::LoadedFolder"]
pub struct LoadedFolder {
    pub handles: Vec<ErasedHandle>,
}

impl Asset for LoadedFolder {}

impl VisitAssetDependencies for LoadedFolder {
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        for handle in &self.handles {
            visit(handle.id());
        }
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

    pub fn erased(self) -> ErasedLoadedAsset {
        ErasedLoadedAsset {
            value: Box::new(self.value),
            dependencies: self.dependencies,
            loader_dependencies: self.loader_dependencies,
            labeled_assets: self.labeled_assets,
            label_to_asset_index: self.label_to_asset_index,
            asset_id_to_asset_index: self.asset_id_to_asset_index,
        }
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
        asset.erased()
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

    pub fn get_mut<A: Asset>(&mut self) -> Option<&mut A> {
        <dyn Any>::downcast_mut::<A>(&mut self.value)
    }

    pub fn asset_type_id(&self) -> TypeId {
        self.value.as_ref().type_id()
    }

    pub fn asset_type_path(&self) -> &'static str {
        self.value.reflect_type_path()
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
    #[expect(clippy::result_large_err, reason = "Err(self) is not a error")]
    pub fn downcast<A: Asset>(self) -> Result<LoadedAsset<A>, ErasedLoadedAsset> {
        if self.value.as_ref().type_id() == TypeId::of::<A>() {
            Ok(LoadedAsset {
                #[expect(unsafe_code, reason = "already checked")]
                value: unsafe { *<Box<dyn Any>>::downcast::<A>(self.value).unwrap_unchecked() },
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

// -----------------------------------------------------------------------------
// LoadedAsset Implementation

#[derive(TypePath)]
#[type_path = "voker_asset::loader::LoadedUntypedAsset"]
pub struct LoadedUntypedAsset {
    /// The handle to the loaded asset.
    pub handle: ErasedHandle,
}

impl Asset for LoadedUntypedAsset {}

impl VisitAssetDependencies for LoadedUntypedAsset {
    #[inline]
    fn visit_dependencies(&self, visit: &mut dyn FnMut(ErasedAssetId)) {
        visit(self.handle.id())
    }
}
