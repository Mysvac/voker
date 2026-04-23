use core::task::Context;

use alloc::vec::Vec;

use atomicow::CowArc;
use voker_utils::hash::{HashMap, HashSet};

use crate::asset::Asset;
use crate::ident::{ErasedAssetId, TypedAssetIndex};
use crate::loader::{LabeledAsset, LoadedAsset};
use crate::meta::AssetHash;
use crate::path::AssetPath;
use crate::server::AssetServer;

pub struct LoadContext<'a> {
    pub(crate) asset_server: &'a AssetServer,
    pub(crate) should_load_dependencies: bool,
    pub(crate) populate_hashes: bool,
    pub(crate) asset_path: AssetPath<'static>,
    pub(crate) dependencies: HashSet<TypedAssetIndex>,
    pub(crate) loader_dependencies: HashMap<AssetPath<'static>, AssetHash>,
    pub(crate) labeled_assets: Vec<LabeledAsset>,
    pub(crate) label_to_asset_index: HashMap<CowArc<'static, str>, usize>,
    pub(crate) asset_id_to_asset_index: HashMap<ErasedAssetId, usize>,
}

impl<'a> LoadContext<'a> {
    #[inline]
    pub(crate) fn new(
        asset_server: &'a AssetServer,
        asset_path: AssetPath<'static>,
        should_load_dependencies: bool,
        populate_hashes: bool,
    ) -> Self {
        Self {
            asset_server,
            asset_path,
            populate_hashes,
            should_load_dependencies,
            dependencies: HashSet::new(),
            loader_dependencies: HashMap::new(),
            labeled_assets: Vec::new(),
            label_to_asset_index: HashMap::new(),
            asset_id_to_asset_index: HashMap::new(),
        }
    }

    pub fn path(&self) -> &AssetPath<'static> {
        &self.asset_path
    }

    pub fn finish<A: Asset>(mut self, value: A) -> LoadedAsset<A> {
        value.visit_dependencies(&mut |asset_id| {
            match asset_id {
                ErasedAssetId::Index { type_id, index } => {
                    self.dependencies.insert(TypedAssetIndex { index, type_id });
                }
                // UUID assets can't be loaded anyway, so just ignore this ID.
                ErasedAssetId::Uuid { .. } => return,
            };
        });

        LoadedAsset {
            value,
            dependencies: self.dependencies,
            loader_dependencies: self.loader_dependencies,
            labeled_assets: self.labeled_assets,
            label_to_asset_index: self.label_to_asset_index,
            asset_id_to_asset_index: self.asset_id_to_asset_index,
        }
    }

    pub fn has_labeled_asset<'b>(&self, label: impl Into<CowArc<'b, str>>) -> bool {
        // let path = self.asset_path.clone().with_label(label.into());
        // !self.asset_server.get_handles_untyped(&path).is_empty()
        todo!()
    }

    pub fn begin_labeled_asset(&self) -> LoadContext<'_> {
        LoadContext::new(
            self.asset_server,
            self.asset_path.clone(),
            self.should_load_dependencies,
            self.populate_hashes,
        )
    }
}
