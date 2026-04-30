#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

// -----------------------------------------------------------------------------
// Android

#[cfg(target_os = "android")]
mod android;

// -----------------------------------------------------------------------------
// Memory

#[cfg(feature = "trace_tracy_memory")]
#[global_allocator]
static GLOBAL: tracy_client::ProfiledAllocator<std::alloc::System> =
    tracy_client::ProfiledAllocator::new(std::alloc::System, 100);

// -----------------------------------------------------------------------------
// Modules

mod once;
mod plugin;

// -----------------------------------------------------------------------------
// Exports

pub use plugin::{BoxedFmtLayer, BoxedLayer, DEFAULT_FILTER, LogPlugin};

pub use tracing::{self, Level, event};
pub use tracing::{debug, error, info, trace, warn};
pub use tracing::{debug_span, error_span, info_span, trace_span, warn_span};
pub use tracing_subscriber;
pub use voker_os::once_expr;

/// The log prelude.
///
/// This is a lightweight import surface for common log macros.
pub mod prelude {
    #[doc(hidden)]
    pub use crate::{debug_once, error_once, info_once, trace_once, warn_once};

    #[doc(hidden)]
    pub use tracing::{debug, error, info, trace, warn};

    #[doc(hidden)]
    pub use tracing::{debug_span, error_span, info_span, trace_span, warn_span};
}
