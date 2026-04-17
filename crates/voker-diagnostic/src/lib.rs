#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![no_std]

// -----------------------------------------------------------------------------
// no_std support

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

// -----------------------------------------------------------------------------
// Modules

mod diagnostic;
mod entity_count;
mod frame_count;
mod log_diagnostic;
#[cfg(feature = "sysinfo_plugin")]
mod system_info;

// -----------------------------------------------------------------------------
// Exports

pub use diagnostic::{Diagnostic, DiagnosticMeasurement, DiagnosticPath};
pub use diagnostic::{DiagnosticsStore, RegisterDiagnostic};
pub use entity_count::{EntityCount, EntityCountDiagnosticsPlugin, EntityCountPlugin};
pub use frame_count::{FrameCount, FrameCountDiagnosticsPlugin, FrameCountPlugin};
pub use log_diagnostic::{LogDiagnosticsPlugin, LogDiagnosticsState};
#[cfg(feature = "sysinfo_plugin")]
pub use system_info::{SystemInfo, SystemInfoDiagnosticsPlugin};

// -----------------------------------------------------------------------------
// DiagnosticsPlugin

use voker_app::{App, Plugin};

/// Adds core diagnostics resources to an app.
#[derive(Default)]
pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiagnosticsStore>();

        #[cfg(feature = "sysinfo_plugin")]
        app.init_resource::<SystemInfo>();
    }
}

/// Default history length for new diagnostics.
pub const DEFAULT_MAX_HISTORY_LENGTH: usize = 120;
