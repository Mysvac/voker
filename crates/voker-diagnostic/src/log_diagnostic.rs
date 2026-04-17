use core::time::Duration;

use log::{debug, info};
use voker_app::prelude::*;
use voker_ecs::borrow::{Res, ResMut};
use voker_ecs::resource::Resource;
use voker_os::time::Instant;
use voker_utils::hash::HashSet;

use crate::{Diagnostic, DiagnosticPath, DiagnosticsStore};

/// Logs diagnostics at a fixed interval.
pub struct LogDiagnosticsPlugin {
    /// If true, logs full debug output for each diagnostic.
    pub debug: bool,
    /// Time to wait between logs.
    pub wait_duration: Duration,
    /// Optional allow-list of diagnostic paths.
    pub filter: Option<HashSet<DiagnosticPath>>,
}

/// Mutable logging state used by [`LogDiagnosticsPlugin`].
#[derive(Resource)]
pub struct LogDiagnosticsState {
    wait_duration: Duration,
    last_log_time: Option<Instant>,
    debug: bool,
    filter: Option<HashSet<DiagnosticPath>>,
}

impl LogDiagnosticsState {
    /// Sets the interval used for periodic logs.
    pub fn set_wait_duration(&mut self, duration: Duration) {
        self.wait_duration = duration;
        self.last_log_time = None;
    }

    /// Adds one path to the allow-list, returning true if inserted.
    pub fn add_filter(&mut self, diagnostic_path: DiagnosticPath) -> bool {
        if let Some(filter) = &mut self.filter {
            filter.insert(diagnostic_path)
        } else {
            self.filter = Some(HashSet::from_iter([diagnostic_path]));
            true
        }
    }

    /// Extends the allow-list with multiple paths.
    pub fn extend_filter(&mut self, iter: impl IntoIterator<Item = DiagnosticPath>) {
        if let Some(filter) = &mut self.filter {
            filter.extend(iter);
        } else {
            self.filter = Some(HashSet::from_iter(iter));
        }
    }

    /// Removes one path from the allow-list.
    pub fn remove_filter(&mut self, diagnostic_path: &DiagnosticPath) -> bool {
        if let Some(filter) = &mut self.filter {
            filter.remove(diagnostic_path)
        } else {
            false
        }
    }

    /// Clears allow-list entries while preserving filtering mode.
    pub fn clear_filter(&mut self) {
        if let Some(filter) = &mut self.filter {
            filter.clear();
        }
    }

    /// Enables filtering with an initially empty allow-list.
    pub fn enable_filtering(&mut self) {
        self.filter = Some(HashSet::new());
    }

    /// Disables filtering.
    pub fn disable_filtering(&mut self) {
        self.filter = None;
    }

    fn should_log(&mut self, now: Instant) -> bool {
        let Some(last) = self.last_log_time else {
            self.last_log_time = Some(now);
            return true;
        };

        if now.duration_since(last) >= self.wait_duration {
            self.last_log_time = Some(now);
            true
        } else {
            false
        }
    }
}

impl Default for LogDiagnosticsPlugin {
    fn default() -> Self {
        Self {
            debug: false,
            wait_duration: Duration::from_secs(1),
            filter: None,
        }
    }
}

impl Plugin for LogDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LogDiagnosticsState {
            wait_duration: self.wait_duration,
            last_log_time: None,
            debug: self.debug,
            filter: self.filter.clone(),
        });

        app.add_systems(PostUpdate, Self::log_diagnostics_system);
    }
}

impl LogDiagnosticsPlugin {
    /// Creates a plugin that logs only diagnostics in `filter`.
    pub fn filtered(filter: HashSet<DiagnosticPath>) -> Self {
        Self {
            filter: Some(filter),
            ..Self::default()
        }
    }

    fn for_each_diagnostic(
        state: &LogDiagnosticsState,
        diagnostics: &DiagnosticsStore,
        mut callback: impl FnMut(&Diagnostic),
    ) {
        if let Some(filter) = &state.filter {
            for path in filter {
                if let Some(diagnostic) = diagnostics.get(path)
                    && diagnostic.is_enabled
                {
                    callback(diagnostic);
                }
            }
        } else {
            for diagnostic in diagnostics.iter() {
                if diagnostic.is_enabled {
                    callback(diagnostic);
                }
            }
        }
    }

    fn log_diagnostic(path_width: usize, diagnostic: &Diagnostic) {
        let Some(value) = diagnostic.smoothed() else {
            return;
        };

        if diagnostic.max_history_length() > 1 {
            let Some(average) = diagnostic.average() else {
                return;
            };

            info!(
                target: "voker_diagnostic",
                "{path:<path_width$}: {value:>11.6}{suffix:2} (avg {average:>.6}{suffix:})",
                path = diagnostic.path(),
                suffix = diagnostic.suffix,
            );
        } else {
            info!(
                target: "voker_diagnostic",
                "{path:<path_width$}: {value:>.6}{suffix:}",
                path = diagnostic.path(),
                suffix = diagnostic.suffix,
            );
        }
    }

    fn log_diagnostics(state: &LogDiagnosticsState, diagnostics: &DiagnosticsStore) {
        let mut path_width = 0;
        Self::for_each_diagnostic(state, diagnostics, |diagnostic| {
            let width = diagnostic.path().as_str().len();
            path_width = path_width.max(width);
        });

        Self::for_each_diagnostic(state, diagnostics, |diagnostic| {
            Self::log_diagnostic(path_width, diagnostic);
        });
    }

    fn log_diagnostics_system(
        mut state: ResMut<LogDiagnosticsState>,
        diagnostics: Res<DiagnosticsStore>,
    ) {
        if !state.should_log(Instant::now()) {
            return;
        }

        if state.debug {
            Self::for_each_diagnostic(&state, &diagnostics, |diagnostic| {
                debug!(target: "voker_diagnostic", "{diagnostic:#?}");
            });
        } else {
            Self::log_diagnostics(&state, &diagnostics);
        }
    }
}
