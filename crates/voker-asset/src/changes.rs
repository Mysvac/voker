use voker_ecs::derive::Resource;
use voker_ecs::tick::Tick;
use voker_utils::hash::HashMap;

use crate::asset::Asset;
use crate::ident::AssetId;

#[derive(Resource)]
pub struct AssetChanges<A: Asset> {
    change_ticks: HashMap<AssetId<A>, Tick>,
    last_change_tick: Tick,
}

impl<A: Asset> AssetChanges<A> {
    pub fn insert(&mut self, asset_id: AssetId<A>, tick: Tick) {
        self.last_change_tick = tick;
        self.change_ticks.insert(asset_id, tick);
    }

    pub fn remove(&mut self, asset_id: &AssetId<A>) {
        self.change_ticks.remove(asset_id);
    }

    pub fn get(&self, asset_id: AssetId<A>) -> Option<Tick> {
        self.change_ticks.get(&asset_id).copied()
    }
}

impl<A: Asset> Default for AssetChanges<A> {
    fn default() -> Self {
        Self {
            change_ticks: HashMap::new(),
            last_change_tick: Tick::new(0),
        }
    }
}
