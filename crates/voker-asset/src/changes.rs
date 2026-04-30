use voker_ecs::derive::Resource;
use voker_ecs::tick::Tick;
use voker_utils::hash::HashMap;

use crate::asset::Asset;
use crate::ident::AssetId;

/// Tracks the [`Tick`] at which each asset of type `A` was last changed.
///
/// Populated by the asset-event system and used by render or other downstream
/// systems to detect which assets need to be re-processed in a given frame.
#[derive(Resource)]
pub struct AssetChanges<A: Asset> {
    change_ticks: HashMap<AssetId<A>, Tick>,
    last_change_tick: Tick,
}

impl<A: Asset> AssetChanges<A> {
    /// Records that `asset_id` changed at `tick`.
    pub fn insert(&mut self, asset_id: AssetId<A>, tick: Tick) {
        self.last_change_tick = tick;
        self.change_ticks.insert(asset_id, tick);
    }

    /// Removes the change record for `asset_id`.
    pub fn remove(&mut self, asset_id: &AssetId<A>) {
        self.change_ticks.remove(asset_id);
    }

    /// Returns the tick at which `asset_id` was last changed, or [`None`] if not tracked.
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
