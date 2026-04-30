# Asset loading, caching, and lifecycle management

`voker-asset` provides a complete, async-first asset pipeline:

- type-erased and typed **handles** with reference-counted lifetime management,
- **`AssetServer`** for async loading, path-level deduplication, hot-reload, and dependency tracking,
- **`Assets<A>`** typed storage resource with `Added`/`Modified`/`Removed`/`Unused`/`FullyLoaded` events,
- **`AssetLoader`** trait for file-format plugins,
- virtual-filesystem abstraction (`AssetSource`) backed by the local filesystem, embedded bytes, HTTP, Android AAssets, or Wasm `fetch`,
- optional **asset processor** pipeline (import / transcode step between raw and processed sources),
- stable **UUID handles** for compile-time asset identity without filesystem paths.

## Concepts

```text
 AssetPath ("textures/player.png")
       │
       ▼
  AssetServer ─── finds AssetLoader by extension ──► loader.load(reader, settings, ctx)
       │                                                         │
       │                                                         ▼
       │                                                  LoadedAsset<A>
       │                                                  (+ labeled sub-assets)
       │
       ▼
  Assets<A>   ──  indexed by AssetId<A>  ──  referenced by Handle<A>
```

| Concept | Type | Description |
|---|---|---|
| Asset type | `Asset` | Trait all loadable data types implement. |
| Handle | `Handle<A>` / `ErasedHandle` | Reference to a live or loading asset. Strong handles keep the asset alive. |
| Identity | `AssetId<A>` / `ErasedAssetId` | Either a generation-indexed slot or a stable UUID. |
| Path | `AssetPath` | `[source://]path/to/file[#label]` string. |
| Storage | `Assets<A>` | Resource holding all loaded assets of type `A`. |
| Server | `AssetServer` | Central coordinator: loads, caches, and tracks assets. |
| Loader | `AssetLoader` | File-format plugin that deserialises bytes into an asset. |
| Source | `AssetSource` | IO backend (filesystem, embedded, HTTP, …). |

## Quick Start

### Add the plugin

```rust, ignore
use voker_app::App;
use voker_asset::plugin::AssetPlugin;

App::new()
    .add_plugins(AssetPlugin::default())
    .run();
```

`AssetPlugin` registers the default file-system source (reads from `assets/` by default),
the `AssetServer` resource, and the background event-flush systems.

### Load an asset

```rust, ignore
use voker_app::App;
use voker_asset::{AssetServer, Handle};
use voker_ecs::borrow::Res;

fn load_texture(server: Res<AssetServer>) -> Handle<MyTexture> {
    server.load("textures/player.png")
}
```

The returned `Handle<MyTexture>` is strong — the asset stays alive as long as any clone
of the handle exists.  The load runs asynchronously; poll
`AssetServer::is_loaded` or listen for `AssetEvent::FullyLoaded` before reading.

### Read a loaded asset

```rust, ignore
use voker_asset::{Assets, Handle};
use voker_ecs::borrow::Res;

fn render(assets: Res<Assets<MyTexture>>, handle: &Handle<MyTexture>) {
    if let Some(tex) = assets.get(handle) {
        // use tex
    }
}
```

### Insert an asset directly

```rust, ignore
use voker_asset::{Assets, Handle};
use voker_ecs::borrow::ResMut;

fn insert(mut assets: ResMut<Assets<MyTexture>>) -> Handle<MyTexture> {
    assets.add(MyTexture { /* … */ })
}
```

## Defining a Custom Asset Type

```rust, ignore
use voker_asset::asset::Asset;
use voker_asset::handle::Handle;
use voker_reflect::prelude::*;

#[derive(Asset, TypePath)]
pub struct MyTexture {
    pub width:  u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
```

Register it with the app so `Assets<MyTexture>` is available and events are wired up:

```rust, ignore
use voker_asset::plugin::AppAssetExt;

app.init_asset::<MyTexture>();
```

## Implementing an Asset Loader

```rust, ignore
use voker_asset::loader::{AssetLoader, LoadContext};
use voker_asset::io::Reader;
use voker_asset::meta::Settings;
use voker_ecs::error::GameError;
use voker_reflect::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct PngLoader;

#[derive(Default, Serialize, Deserialize)]
pub struct PngSettings;
impl Settings for PngSettings {}

impl AssetLoader for PngLoader {
    type Asset    = MyTexture;
    type Settings = PngSettings;
    type Error    = GameError;

    async fn load(
        &self,
        reader:  &mut dyn Reader,
        _settings: &Self::Settings,
        _ctx:    &mut LoadContext<'_>,
    ) -> Result<MyTexture, GameError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await.map_err(GameError::new)?;
        // decode bytes → MyTexture …
        Ok(MyTexture { width: 0, height: 0, pixels: bytes })
    }

    fn extensions(&self) -> &[&'static str] {
        &["png"]
    }
}
```

Register with the app:

```rust, ignore
app.register_asset_loader(PngLoader);
```

## Asset Paths

Paths follow the format `[source://]path/to/file[#label]`:

| Example | Meaning |
|---|---|
| `"textures/player.png"` | File in the default source. |
| `"embedded://icons/back.png"` | File in the `embedded` source. |
| `"models/level.gltf#Mesh0"` | Sub-asset (label) inside `level.gltf`. |

```rust, ignore
use voker_asset::path::AssetPath;

let path = AssetPath::parse("models/level.gltf#Mesh0");
assert_eq!(path.path().to_str().unwrap(), "models/level.gltf");
assert_eq!(path.label(), Some("Mesh0"));
```

## Stable UUID Handles

Use `uuid_handle!` or `AssetId::Uuid` to reference assets at compile time without a
filesystem path.  UUID handles never keep an asset alive (no reference counting):

```rust, ignore
use voker_asset::{Handle, uuid_handle};

type MyFont = ();
const DEFAULT_FONT: Handle<MyFont> = uuid_handle!("7e7c4b6e-4d4a-4e4b-8d4c-4e4b8d4c4e4b");
```

## Asset Events

Subscribe to `AssetEvent<A>` messages from any system:

```rust, ignore
use voker_asset::event::AssetEvent;
use voker_ecs::message::MessageReader;

fn on_texture_changed(
    mut events: MessageReader<AssetEvent<MyTexture>>,
) {
    for event in events.read() {
        match event {
            AssetEvent::Added     { id } => { /* first time loaded */ }
            AssetEvent::Modified  { id } => { /* value replaced   */ }
            AssetEvent::Removed   { id } => { /* removed from store */ }
            AssetEvent::Unused    { id } => { /* last handle dropped */ }
            AssetEvent::FullyLoaded { id } => { /* asset + all deps ready */ }
        }
    }
}
```

## Load State

Poll load state from `AssetServer`:

```rust, ignore
use voker_asset::server::{AssetServer, LoadState};
use voker_ecs::borrow::Res;

fn check(server: Res<AssetServer>, handle: &Handle<MyTexture>) {
    match server.load_state(handle.id()) {
        LoadState::NotLoaded  => {}
        LoadState::Loading    => {}
        LoadState::Loaded     => { /* ready */ }
        LoadState::Failed(_)  => { /* error */ }
    }
}
```

Or `await` it directly in an async context:

```rust, ignore
server.wait_for_asset(&handle).await.unwrap();
```

## Embedded Assets

Embed asset bytes directly into the binary using the `embedded` source:

```rust, ignore
use voker_asset::io::embedded::EmbeddedAssetRegistry;

fn register(embedded: &mut EmbeddedAssetRegistry) {
    embedded.insert_asset(
        std::path::PathBuf::from("icons/back.png"),
        include_bytes!("../assets/icons/back.png"),
    );
}
```

Load with the `embedded://` prefix: `server.load("embedded://icons/back.png")`.

## Hot Reload

Enable the `file_watcher` feature flag.  The asset server detects filesystem changes
and re-runs the appropriate loader, emitting `AssetEvent::Modified` when complete.

## Feature Flags

| Feature | Description |
|---|---|
| `file_watcher` | Watches the asset folder for changes and hot-reloads modified assets. |
| `asset_processor` | Enables the asset processor pipeline (raw → processed import step). |
| `http` | Adds an HTTP asset source backed by `ureq` (plain HTTP). |
| `https` | Same as `http` with TLS via `rustls` and platform certificate verification. |
| `trace` | Enables tracing instrumentation in the asset pipeline. |
