use voker_ecs::derive::Resource;
use voker_os::Arc;

#[derive(Resource, Clone)]
pub struct AssetServer {
    pub(crate) data: Arc<AssetServerData>,
}

pub(crate) struct AssetServerData {
    // TODO
}
