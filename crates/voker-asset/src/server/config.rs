use voker_utils::hash::HashSet;

use crate::path::AssetPath;

/// Selects whether the asset server reads from raw sources or processed/imported sources.
///
/// - `Unprocessed` (default): assets are read directly from the file system source
///   (e.g. `assets/`).  No import step is applied.
/// - `Processed`: assets are read from the processed output folder
///   (default `imported_assets/default`).  Requires the `asset_processor` feature.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetServerMode {
    #[default]
    Unprocessed,
    Processed,
}

/// Controls how the asset server handles paths that escape their source root (e.g. `../`).
///
/// - `Allow`: unapproved paths are loaded without any special treatment.
/// - `Deny` (default): unapproved paths emit a warning but still load.
/// - `Forbid`: unapproved paths immediately fail with an error.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub enum UnapprovedPathMode {
    Allow,
    #[default]
    Deny,
    Forbid,
}

/// Controls when the asset server reads `.meta` sidecar files.
///
/// - `Always` (default): every asset load checks for a `.meta` file and, if found,
///   uses it to select the loader and settings.
/// - `Paths(set)`: only read `.meta` for the specified paths.
/// - `Never`: skip `.meta` checks entirely; always use default settings and
///   extension-based loader selection.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub enum MetaCheckMode {
    #[default]
    Always,
    Paths(HashSet<AssetPath<'static>>),
    Never,
}
