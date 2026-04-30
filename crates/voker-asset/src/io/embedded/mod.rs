mod registry;
pub use registry::*;

/// The name of the `embedded` [`AssetSource`].
///
/// [`AssetSource`]: crate::io::AssetSource
pub const EMBEDDED: &str = "embedded";

/// Load an [embedded asset](embedded_asset).
///
/// This is useful if the embedded asset in question is not publicly exposed,
/// but you need to use it internally.
///
/// # Syntax
///
/// This macro takes two arguments and an optional third one:
/// 1. The asset source. It may be `AssetServer`, `World` , `App` or `SubApp`.
/// 2. The path to the asset to embed, as a string literal.
/// 3. Optionally, a closure of the same type as in [`LoadBuilder::with_settings`].
///    Consider explicitly typing the closure argument in case of type error.
///
/// # Usage
///
/// The advantage compared to using directly [`AssetServer::load`] is:
/// - This also accepts [`World`] and [`App`] arguments.
/// - This uses the exact same path as `embedded_asset!`, so you can keep it consistent.
///
/// - If the asset in used in the same module as it is declared using [`embedded_asset!`],
///   use this macro.
/// - Otherwise, use [`AssetServer::load`].
///
/// [`World`]: voker_ecs::world::World
/// [`App`]: voker_app::App
///
/// # Examples
///
/// ```ignore
/// let handle: Handle<Image> = load_embedded_asset!("textures/icon.png", app);
/// ```
///
/// [`LoadBuilder::with_settings`]: crate::server::LoadBuilder::with_settings
#[macro_export]
macro_rules! load_embedded_asset {
    (@get: $path: literal, $provider: expr) => {{
        let path = $crate::embedded_path!($path);
        let path = $crate::path::AssetPath::from_path_buf(path).with_source("embedded");
        let asset_server = $crate::io::embedded::__GetAssetServer__::get($provider);
        (path, asset_server)
    }};
    ($provider: expr, $path: literal, $settings: expr) => {{
        let (path, asset_server) = $crate::load_embedded_asset!(@get: $path, $provider);
        asset_server.load_builder().with_settings($settings).load(path)
    }};
    ($provider: expr, $path: literal) => {{
        let (path, asset_server) = $crate::load_embedded_asset!(@get: $path, $provider);
        asset_server.load(path)
    }};
}

/// Returns the [`Path`] for a given `embedded` asset.
///
/// This is used internally by [`embedded_asset`] and can be used to
/// get a [`Path`] that matches the [`AssetPath`] used by that asset.
///
/// [`AssetPath`]: crate::path::AssetPath
#[macro_export]
macro_rules! embedded_path {
    ($path_str: expr) => {{ $crate::embedded_path!("src", $path_str) }};
    ($source_path: expr, $path_str: expr) => {{
        let crate_name = module_path!().split(':').next().unwrap();
        $crate::io::embedded::__embedded_asset_path(
            crate_name,
            $source_path.as_ref(),
            file!().as_ref(),
            $path_str.as_ref(),
        )
    }};
}

/// Creates a new `embedded` asset by embedding the bytes of the given path into the current binary
/// and registering those bytes with the `embedded` [`AssetSource`](crate::io::AssetSource).
///
/// By default this will generate an [`AssetPath`](crate::path::AssetPath) using the following rules:
///
/// 1. Search for the first `$crate_name/src/` in the path and trim to the path past that point.
/// 2. Re-add the current `$crate_name` to the front of the path
///
/// For example, consider the following file structure in the theoretical `rock` crate
///
/// ```text
/// rock
/// ├── src
/// │   ├── render
/// │   │   ├── rock.wgsl
/// │   │   └── mod.rs
/// │   └── lib.rs
/// └── Cargo.toml
/// ```
///
/// `rock.wgsl` is a WGSL shader asset, we invoke the following in `bevy_rock/src/render/mod.rs`:
///
/// `embedded_asset!(app, "rock.wgsl")`
///
/// Then `rock.wgsl` can now be loaded by the [`AssetServer`] as follows:
///
/// ```ignore
/// // If we are loading the shader in the same module we used `embedded_asset!`:
/// let shader = load_embedded_asset!(&asset_server, "rock.wgsl");
///
/// // If the goal is to expose the asset **to the end user**:
/// let shader = asset_server.load::<Shader>("embedded://bevy_rock/render/rock.wgsl");
/// ```
///
/// Some things to note in the path:
/// 1. The non-default `embedded://` [`AssetSource`]
/// 2. `src` is trimmed from the path
///
/// This macro uses the [`include_bytes`] macro internally and _will not_ reallocate the
/// bytes. Generally the [`AssetPath`] generated will be predictable, but if your asset
/// isn't available for some reason, you can use the [`embedded_path`] macro to debug.
///
/// Hot-reloading `embedded` assets is supported. Just enable the `file_watcher` cargo feature.
///
/// [`AssetPath`]: crate::path::AssetPath
/// [`AssetSource`]: crate::io::AssetSource
#[macro_export]
macro_rules! embedded_asset {
    ($app: expr, $path: expr) => {
        $crate::embedded_asset!($app, "src", $path)
    };
    ($app: expr, $source_path: expr, $path: expr) => {{
        let mut embedded = $app
            .world_mut()
            .resource_mut::<$crate::io::embedded::EmbeddedAssetRegistry>();
        let path = $crate::embedded_path!($source_path, $path);
        let watched_path = $crate::io::embedded::watched_path(file!(), $path);
        embedded.insert_asset(watched_path, &path, include_bytes!($path));
    }};
}

/// Loads an "internal" asset by embedding the string stored in
/// the given `path_str` and associates it with the given handle.
///
///
/// # Examples
///
/// ```ignore
/// load_internal_asset!(
///     app,
///     shader_handle,
///     "shaders/default.wgsl",
///     |src, path| Shader::from_wgsl(src, path)
/// );
/// ```
#[macro_export]
macro_rules! load_internal_asset {
    ($app: ident, $handle: expr, $path_str: expr, $loader: expr) => {{
        let mut assets = $app.world_mut().resource_mut::<$crate::Assets<_>>();
        assets.insert($handle.id(), ($loader)(
            include_str!($path_str),
            ::std::path::Path::new(file!())
                .parent()
                .unwrap()
                .join($path_str)
                .to_string_lossy()
        )).unwrap();
    }};
    // we can't support params without variadic arguments, so internal assets with additional params can't be hot-reloaded
    ($app: ident, $handle: ident, $path_str: expr, $loader: expr $(, $param:expr)+) => {{
        let mut assets = $app.world_mut().resource_mut::<$crate::Assets<_>>();
        assets.insert(
            $handle.id(),
            ($loader)(
                include_str!($path_str),
                ::std::path::Path::new(file!())
                    .parent()
                    .unwrap()
                    .join($path_str)
                    .to_string_lossy(),
                $($param),+
            )
        ).unwrap();
    }};
}

/// Loads an "internal" binary asset by embedding the bytes stored in
/// the given `path_str` and associates it with the given handle.
///
/// # Examples
///
/// ```ignore
/// load_internal_binary_asset!(
///    app,                           // App Instance
///    image_handle,                  // Created Handle
///    "textures/icon.png",           // Relative path of files
///    |data: &[u8], path: String| -> Image {
///        Image::from_buffer(data, ImageType::Extension("png"))
///    } // Loader function
///);
///```
#[macro_export]
macro_rules! load_internal_binary_asset {
    ($app: ident, $handle: expr, $path_str: expr, $loader: expr) => {{
        let mut assets = $app.world_mut().resource_mut::<$crate::Assets<_>>();
        assets
            .insert(
                $handle.id(),
                ($loader)(
                    include_bytes!($path_str).as_ref(),
                    ::std::path::Path::new(file!())
                        .parent()
                        .unwrap()
                        .join($path_str)
                        .to_string_lossy()
                        .into(),
                ),
            )
            .unwrap();
    }};
}

use std::path::{Path, PathBuf};

/// Returns the path used by the watcher.
#[doc(hidden)]
#[cfg(feature = "file_watcher")]
pub fn watched_path(source_file_path: &'static str, asset_path: &'static str) -> PathBuf {
    PathBuf::from(source_file_path).parent().unwrap().join(asset_path)
}

/// Returns an empty PathBuf.
#[doc(hidden)]
#[cfg(not(feature = "file_watcher"))]
pub fn watched_path(_source_file_path: &'static str, _asset_path: &'static str) -> PathBuf {
    PathBuf::from("")
}

#[doc(hidden)]
pub fn __embedded_asset_path(
    crate_name: &str,
    src_prefix: &Path,
    file_path: &Path,
    asset_path: &Path,
) -> PathBuf {
    let file_path = if cfg!(not(target_family = "windows")) {
        // Windows -> WASM, need to replace the delimiter.
        PathBuf::from(file_path.to_str().unwrap().replace("\\", "/"))
    } else {
        PathBuf::from(file_path)
    };
    let mut maybe_parent = file_path.parent();
    let after_src = loop {
        let Some(parent) = maybe_parent else {
            panic!("Failed to find src_prefix {src_prefix:?} in {file_path:?}")
        };
        if parent.ends_with(src_prefix) {
            break file_path.strip_prefix(parent).unwrap();
        }
        maybe_parent = parent.parent();
    };
    let asset_path = after_src.parent().unwrap().join(asset_path);
    Path::new(crate_name).join(asset_path)
}

use crate::server::AssetServer;

/// Trait for the [`load_embedded_asset!`] macro, to
/// access [`AssetServer`] from arbitrary things.
#[doc(hidden)]
pub trait __GetAssetServer__ {
    fn get(this: &Self) -> &AssetServer;
}

impl __GetAssetServer__ for AssetServer {
    fn get(this: &Self) -> &AssetServer {
        this
    }
}

impl __GetAssetServer__ for voker_app::App {
    fn get(this: &Self) -> &AssetServer {
        __GetAssetServer__::get(this.world())
    }
}

impl __GetAssetServer__ for voker_app::SubApp {
    fn get(this: &Self) -> &AssetServer {
        __GetAssetServer__::get(this.world())
    }
}

impl __GetAssetServer__ for voker_ecs::world::World {
    #[inline]
    fn get(this: &Self) -> &AssetServer {
        this.resource::<AssetServer>()
    }
}
