use core::sync::atomic::Ordering;

use voker_app::{DuplicateStrategy, prelude::*};
use voker_ecs::borrow::{Res, ResMut};
use voker_ecs::resource::Resource;
use voker_ecs::system::Local;
use voker_os::sync::atomic::AtomicU32;
use voker_os::time::Instant;

use crate::{DEFAULT_MAX_HISTORY_LENGTH, RegisterDiagnostic};
use crate::{Diagnostic, DiagnosticPath, DiagnosticsStore};

// -----------------------------------------------------------------------------
// FrameCount & FrameCountPlugin

#[derive(Debug, Default, Resource)]
pub struct FrameCount(AtomicU32);

impl FrameCount {
    pub fn get(&self) -> u32 {
        self.0.load(Ordering::Relaxed)
    }
}

fn count_frame(count: Res<FrameCount>) {
    count.0.fetch_add(1, Ordering::Relaxed);
}

#[derive(Debug, Default)]
pub struct FrameCountPlugin;

impl Plugin for FrameCountPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameCount>();
        app.add_system(Last, count_frame);
    }

    fn duplicate_strategy(&self) -> DuplicateStrategy {
        DuplicateStrategy::Skip
    }
}

// -----------------------------------------------------------------------------
// FrameCountDiagnosticsPlugin

/// Adds `frame_time`, `fps`, and `frame_count` diagnostics.
pub struct FrameCountDiagnosticsPlugin {
    /// Number of samples kept in history.
    pub max_history_length: usize,
    /// Smoothing factor used by exponential moving average.
    pub smoothing_factor: f64,
}

impl Default for FrameCountDiagnosticsPlugin {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_HISTORY_LENGTH)
    }
}

impl FrameCountDiagnosticsPlugin {
    /// Creates a plugin using the provided history length.
    pub fn new(max_history_length: usize) -> Self {
        Self {
            max_history_length,
            smoothing_factor: 2.0 / (max_history_length as f64 + 1.0),
        }
    }
}

impl FrameCountDiagnosticsPlugin {
    /// Frames per second.
    pub const FPS: DiagnosticPath = DiagnosticPath::new("fps");

    /// Total frames since app start.
    pub const FRAME_COUNT: DiagnosticPath = DiagnosticPath::new("frame_count");

    /// Frame time in milliseconds.
    pub const FRAME_TIME: DiagnosticPath = DiagnosticPath::new("frame_time");

    /// Samples `frame_count`, `frame_time`, and `fps` every update.
    fn diagnostic_system(
        mut diagnostics: ResMut<DiagnosticsStore>,
        frame_count: Res<FrameCount>,
        mut last_update: Local<Option<Instant>>,
    ) {
        diagnostics.add_measurement(&Self::FRAME_COUNT, frame_count.get() as f64);

        let now = Instant::now();
        let Some(last) = *last_update else {
            *last_update = Some(now);
            return;
        };

        let delta_seconds = now.duration_since(last).as_secs_f64();
        if delta_seconds > 0.0 {
            diagnostics.add_measurement(&Self::FRAME_TIME, delta_seconds * 1000.0);
            diagnostics.add_measurement(&Self::FPS, 1.0 / delta_seconds);
        }

        *last_update = Some(now);
    }
}

impl Plugin for FrameCountDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        if app.is_plugin_added::<FrameCountPlugin>() {
            app.add_plugins(FrameCountPlugin);
        }

        app.register_diagnostic(
            Diagnostic::new(Self::FRAME_TIME)
                .with_suffix("ms")
                .with_max_history_length(self.max_history_length)
                .with_smoothing_factor(self.smoothing_factor),
        )
        .register_diagnostic(
            Diagnostic::new(Self::FPS)
                .with_max_history_length(self.max_history_length)
                .with_smoothing_factor(self.smoothing_factor),
        )
        .register_diagnostic(
            Diagnostic::new(Self::FRAME_COUNT)
                .with_smoothing_factor(0.0)
                .with_max_history_length(0),
        )
        .add_systems(Update, Self::diagnostic_system);
    }
}
