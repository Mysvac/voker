#![cfg_attr(docsrs, feature(doc_cfg))]
#![no_std]

extern crate alloc;
extern crate std;

// -----------------------------------------------------------------------------
// Modules

mod render_asset;
mod utils;

pub mod asset;
pub mod assets;
pub mod changes;
pub mod direct_access_ext;
pub mod event;
pub mod handle;
pub mod ident;
pub mod io;
pub mod loader;
pub mod meta;
pub mod path;
pub mod plugin;
pub mod processor;
pub mod saver;
pub mod server;
pub mod transformer;

// -----------------------------------------------------------------------------
// Exports

pub use asset::{Asset, AssetComponent, VisitAssetDependencies};
pub use assets::Assets;
pub use changes::AssetChanges;
pub use handle::{ErasedHandle, Handle};
pub use render_asset::RenderAssetUsages;
pub use server::AssetServer;
pub use utils::{BoxedFuture, EmptyPathStream, PathStream};
pub use uuid;
