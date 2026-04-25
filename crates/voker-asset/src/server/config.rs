use voker_utils::hash::HashSet;

use crate::path::AssetPath;

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetServerMode {
    #[default]
    Unprocessed,
    Processed,
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub enum UnapprovedPathMode {
    Allow,
    #[default]
    Deny,
    Forbid,
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub enum MetaCheckMode {
    #[default]
    Always,
    Paths(HashSet<AssetPath<'static>>),
    Never,
}
