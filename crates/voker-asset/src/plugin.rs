use core::any::TypeId;

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;

use alloc::sync::Arc;
use voker_app::{App, Plugin, PostUpdate, PreUpdate, Startup};
use voker_ecs::schedule::{IntoSystemConfig, IntoSystemSetConfig};
use voker_ecs::world::FromWorld;

use crate::asset::{Asset, AssetEventSystems, AssetServerEventSystems, AssetTrackingSystems};
use crate::assets::Assets;
use crate::event::{AssetEvent, AssetLoadFailedEvent, ErasedAssetLoadFailedEvent};
use crate::handle::AssetHandleProvider;
use crate::ident::{AssetIndexAllocator, AssetSourceId};
use crate::io::embedded::EmbeddedAssetRegistry;
use crate::io::{AssetSourceBuilder, AssetSourceBuilders};
use crate::loader::{AssetLoader, LoadedFolder, LoadedUntypedAsset};
use crate::processor::{AssetProcessServer, AssetProcessor, TransactionLogFactory};
use crate::server::{
    AssetServer, AssetServerMode, MetaCheckMode, UnapprovedPathMode, handle_asset_server_events,
};

pub const DEFAULT_PROCESSED_FILE_PATH: &str = "imported_assets/default";
pub const DEFAULT_UNPROCESSED_FILE_PATH: &str = "assets";

pub trait AppAssetExt {
    fn preregister_asset_loader<L: AssetLoader>(
        &mut self,
        extensions: &[&'static str],
    ) -> &mut Self;

    fn register_asset_loader<L: AssetLoader>(&mut self, loader: L) -> &mut Self;

    fn register_asset_processor<P: AssetProcessor>(&mut self, processor: P) -> &mut Self;

    fn register_asset_source(
        &mut self,
        id: impl Into<AssetSourceId<'static>>,
        source: AssetSourceBuilder,
    ) -> &mut Self;

    fn set_default_asset_processor<P: AssetProcessor>(&mut self, extension: &str) -> &mut Self;

    fn init_asset_loader<L: AssetLoader + FromWorld>(&mut self) -> &mut Self;

    fn init_asset<A: Asset>(&mut self) -> &mut Self;
}

impl AppAssetExt for App {
    fn preregister_asset_loader<L: AssetLoader>(
        &mut self,
        extensions: &[&'static str],
    ) -> &mut Self {
        let server = self.world().resource::<AssetServer>();
        server.preregister_loader::<L>(extensions);
        self
    }

    fn register_asset_loader<L: AssetLoader>(&mut self, loader: L) -> &mut Self {
        let server = self.world().resource::<AssetServer>();
        server.register_loader(loader);
        self
    }

    fn register_asset_processor<P: AssetProcessor>(&mut self, processor: P) -> &mut Self {
        if let Some(s) = self.world().get_resource::<AssetProcessServer>() {
            s.register_processor(processor);
        }
        self
    }

    fn init_asset<A: Asset>(&mut self) -> &mut Self {
        let assets = Assets::<A>::default();
        let world = self.world_mut();
        world.resource::<AssetServer>().register_asset(&assets);

        if let Some(server) = world.get_resource::<AssetProcessServer>() {
            let provider =
                AssetHandleProvider::new(TypeId::of::<A>(), Arc::new(AssetIndexAllocator::new()));
            server.server().register_handle_provider(provider);
        }

        world.insert_resource(assets);
        world.register_message::<AssetEvent<A>>();
        world.register_message::<AssetLoadFailedEvent<A>>();

        world.add_systems(
            PostUpdate,
            AssetEventSystems,
            Assets::<A>::handle_asset_events.run_if(Assets::<A>::asset_events_condition),
        );

        world.add_system(PreUpdate, AssetTrackingSystems, Assets::<A>::track_assets);

        self
    }

    fn init_asset_loader<L: AssetLoader + FromWorld>(&mut self) -> &mut Self {
        let loader = L::from_world(self.world_mut());
        self.register_asset_loader(loader)
    }

    fn set_default_asset_processor<P: AssetProcessor>(&mut self, extension: &str) -> &mut Self {
        if let Some(s) = self.world().get_resource::<AssetProcessServer>() {
            s.set_default_processor::<P>(extension);
        }
        self
    }

    fn register_asset_source(
        &mut self,
        id: impl Into<AssetSourceId<'static>>,
        source: AssetSourceBuilder,
    ) -> &mut Self {
        let id = id.into();
        let world = self.world_mut();

        if world.get_resource::<AssetServer>().is_some() {
            tracing::error!(
                "{} must be registered before `AssetPlugin` \
                (typically added as part of `DefaultPlugins`)",
                id
            );
        }

        world.resource_mut_or_init::<AssetSourceBuilders>().insert(id, source);

        self
    }
}

pub struct AssetPlugin {
    pub file_path: String,
    pub processed_file_path: String,
    pub watch_for_changes_override: Option<bool>,
    pub use_asset_processor_override: Option<bool>,
    pub transaction_log_factory: Option<fn() -> Box<dyn TransactionLogFactory>>,
    pub server_mode: AssetServerMode,
    pub meta_check_mode: MetaCheckMode,
    pub unapproved_path_mode: UnapprovedPathMode,
}

impl Default for AssetPlugin {
    fn default() -> Self {
        Self {
            server_mode: AssetServerMode::Unprocessed,
            file_path: DEFAULT_UNPROCESSED_FILE_PATH.to_owned(),
            processed_file_path: DEFAULT_PROCESSED_FILE_PATH.to_owned(),
            transaction_log_factory: None,
            watch_for_changes_override: None,
            use_asset_processor_override: None,
            meta_check_mode: MetaCheckMode::default(),
            unapproved_path_mode: UnapprovedPathMode::default(),
        }
    }
}

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        let embedded = EmbeddedAssetRegistry::default();

        // init + embedded
        let world = app.world_mut();
        let builders = world.resource_mut_or_init::<AssetSourceBuilders>();
        let builders = builders.into_inner();
        match self.server_mode {
            AssetServerMode::Unprocessed => builders.init_default_source(&self.file_path, None),
            AssetServerMode::Processed => {
                let processed = self.processed_file_path.as_str();
                builders.init_default_source(&self.file_path, Some(processed))
            }
        }
        embedded.register_source(builders);

        // read config
        let watching_for_changes = self
            .watch_for_changes_override
            .unwrap_or(cfg!(feature = "file_watcher"));
        let asset_processor = self
            .use_asset_processor_override
            .unwrap_or(cfg!(feature = "asset_processor"));

        match self.server_mode {
            AssetServerMode::Unprocessed => {
                let sources = builders.build_sources(watching_for_changes, false);
                world.insert_resource(AssetServer::new(
                    Arc::new(sources),
                    AssetServerMode::Unprocessed,
                    MetaCheckMode::Always,
                    watching_for_changes,
                    self.unapproved_path_mode.clone(),
                ));
            }
            AssetServerMode::Processed if asset_processor => {
                let transaction_log_factory = self.transaction_log_factory.map(|f| f());
                let (processors, sources) = AssetProcessServer::build(
                    builders,
                    watching_for_changes,
                    transaction_log_factory,
                );
                world.insert_resource(processors);
                // Note that we need an independent AssetServer
                // and cannot directly use server within ProcessorServer.
                world.insert_resource(AssetServer::new(
                    sources,
                    AssetServerMode::Processed,
                    MetaCheckMode::Always,
                    watching_for_changes,
                    self.unapproved_path_mode.clone(),
                ));
                world.add_system(Startup, (), AssetProcessServer::start);
            }
            _ => {
                let sources = builders.build_sources(false, watching_for_changes);
                world.insert_resource(AssetServer::new(
                    Arc::new(sources),
                    AssetServerMode::Processed,
                    MetaCheckMode::Always,
                    watching_for_changes,
                    self.unapproved_path_mode.clone(),
                ));
            }
        }

        world.insert_resource(embedded);
        world.register_message::<ErasedAssetLoadFailedEvent>();

        world.add_system(
            PreUpdate,
            AssetServerEventSystems,
            handle_asset_server_events,
        );

        world
            .schedule_entry(PreUpdate)
            .config_set(AssetTrackingSystems.run_after(AssetServerEventSystems));

        app.init_asset::<LoadedFolder>()
            .init_asset::<LoadedUntypedAsset>()
            .init_asset::<()>();
    }

    fn cleanup(&mut self, _app: &mut App) {
        *self = Self::default(); // drop memory.
    }
}
