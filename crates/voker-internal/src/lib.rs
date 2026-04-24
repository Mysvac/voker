//! Features integration layer
#![no_std]

pub use voker_cfg as cfg;
pub use voker_ecs as ecs;
pub use voker_inventory as inventory;
pub use voker_math as math;
pub use voker_os as os;
pub use voker_ptr as ptr;
pub use voker_reflect as reflect;
pub use voker_state as state;
pub use voker_task as task;
pub use voker_utils as utils;

#[cfg(feature = "voker-asset")]
pub use voker_asset as asset;

#[cfg(feature = "voker-log")]
pub use voker_log as log;

#[cfg(feature = "voker-diagnostic")]
pub use voker_diagnostic as diagnostic;
