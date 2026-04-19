use alloc::{borrow::Cow, collections::VecDeque, string::String};
use core::time::Duration;

use voker_app::{App, SubApp};
use voker_ecs::resource::Resource;
use voker_os::time::Instant;
use voker_utils::hash::HashMap;

use crate::DEFAULT_MAX_HISTORY_LENGTH;

// -----------------------------------------------------------------------------
// DiagnosticPath

/// Unique diagnostic path, separated by `/`.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct DiagnosticPath(Cow<'static, str>);

impl DiagnosticPath {
    /// Creates a `DiagnosticPath` in const contexts.
    #[track_caller]
    pub const fn new(path: &'static str) -> Self {
        assert!(!path.is_empty(), "diagnostic path should not be empty");
        Self(Cow::Borrowed(path))
    }

    /// Creates a `DiagnosticPath` from a string.
    pub fn from_path(path: impl Into<Cow<'static, str>>) -> Self {
        let path = path.into();

        debug_assert!(!path.is_empty(), "diagnostic path should not be empty");
        debug_assert!(
            !path.starts_with('/'),
            "diagnostic path should not start with `/`: \"{path}\""
        );
        debug_assert!(
            !path.ends_with('/'),
            "diagnostic path should not end with `/`: \"{path}\""
        );
        debug_assert!(
            !path.contains("//"),
            "diagnostic path should not contain empty components: \"{path}\""
        );

        Self(path)
    }

    /// Returns the full slash-separated path string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Creates a path from slash-joined components.
    pub fn from_components<'a>(components: impl IntoIterator<Item = &'a str>) -> Self {
        let mut buf = String::new();

        for (i, component) in components.into_iter().enumerate() {
            if i > 0 {
                buf.push('/');
            }
            buf.push_str(component);
        }

        Self::from_path(buf)
    }

    /// Iterates path components.
    pub fn components(&self) -> impl Iterator<Item = &str> + '_ {
        self.0.split('/')
    }
}

impl From<DiagnosticPath> for String {
    fn from(path: DiagnosticPath) -> Self {
        path.0.into_owned()
    }
}

impl core::fmt::Display for DiagnosticPath {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

// -----------------------------------------------------------------------------
// DiagnosticMeasurement

/// A single sampled value at a point in time.
#[derive(Debug)]
pub struct DiagnosticMeasurement {
    /// Timestamp of this sample.
    pub time: Instant,
    /// Sample value.
    pub value: f64,
}

// -----------------------------------------------------------------------------
// Diagnostic

/// A timeline of sampled values for a single diagnostic metric.
#[derive(Debug)]
pub struct Diagnostic {
    path: DiagnosticPath,
    /// Optional textual suffix, e.g. `%` or `ms`.
    pub suffix: Cow<'static, str>,
    history: VecDeque<DiagnosticMeasurement>,
    sum: f64,
    ema: f64,
    ema_smoothing_factor: f64,
    max_history_length: usize,
    /// Disabled diagnostics are ignored for logging and measurement updates.
    pub is_enabled: bool,
}

impl Diagnostic {
    /// Creates a new diagnostic with default history and smoothing behavior.
    pub fn new(path: DiagnosticPath) -> Self {
        Self {
            path,
            suffix: Cow::Borrowed(""),
            history: VecDeque::with_capacity(DEFAULT_MAX_HISTORY_LENGTH),
            max_history_length: DEFAULT_MAX_HISTORY_LENGTH,
            sum: 0.0,
            ema: 0.0,
            ema_smoothing_factor: 2.0 / 21.0,
            is_enabled: true,
        }
    }

    /// Configures the history length used for averaging.
    #[must_use]
    pub fn with_max_history_length(mut self, max_history_length: usize) -> Self {
        self.max_history_length = max_history_length;
        self
    }

    /// Configures a display suffix for this diagnostic.
    #[must_use]
    pub fn with_suffix(mut self, suffix: impl Into<Cow<'static, str>>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// Configures the exponential moving-average smoothing factor in seconds.
    #[must_use]
    pub fn with_smoothing_factor(mut self, smoothing_factor: f64) -> Self {
        self.ema_smoothing_factor = smoothing_factor;
        self
    }

    /// Returns the diagnostic path key.
    pub fn path(&self) -> &DiagnosticPath {
        &self.path
    }

    /// Returns the latest measurement.
    pub fn measurement(&self) -> Option<&DiagnosticMeasurement> {
        self.history.back()
    }

    /// Returns the latest scalar value.
    pub fn value(&self) -> Option<f64> {
        self.measurement().map(|measurement| measurement.value)
    }

    /// Returns the simple moving average over stored history.
    pub fn average(&self) -> Option<f64> {
        if self.history.is_empty() {
            None
        } else {
            Some(self.sum / self.history.len() as f64)
        }
    }

    /// Returns the exponential moving average over stored history.
    pub fn smoothed(&self) -> Option<f64> {
        if self.history.is_empty() {
            None
        } else {
            Some(self.ema)
        }
    }

    /// Returns the current number of stored samples.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Returns the time spanned by stored samples.
    pub fn duration(&self) -> Option<Duration> {
        if self.history.len() < 2 {
            return None;
        }

        let newest = self.history.back()?;
        let oldest = self.history.front()?;
        Some(newest.time.duration_since(oldest.time))
    }

    /// Returns the configured maximum history length.
    pub fn max_history_length(&self) -> usize {
        self.max_history_length
    }

    /// Returns an iterator over measured values.
    pub fn values(&self) -> impl Iterator<Item = &f64> {
        self.history.iter().map(|x| &x.value)
    }

    /// Returns an iterator over measurements.
    pub fn measurements(&self) -> impl Iterator<Item = &DiagnosticMeasurement> {
        self.history.iter()
    }

    /// Clears all historical measurements and cached aggregates.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.sum = 0.0;
        self.ema = 0.0;
    }

    /// Appends a new measurement and updates moving statistics.
    pub fn add_measurement(&mut self, measurement: DiagnosticMeasurement) {
        if measurement.value.is_nan() {
            // Keep previous EMA when sample is not a number.
        } else if let Some(previous) = self.measurement() {
            let delta = (measurement.time - previous.time).as_secs_f64();
            let alpha = (delta / self.ema_smoothing_factor).clamp(0.0, 1.0);
            self.ema += alpha * (measurement.value - self.ema);
        } else {
            self.ema = measurement.value;
        }

        if self.max_history_length > 1 {
            if self.history.len() >= self.max_history_length
                && let Some(removed) = self.history.pop_front()
                && !removed.value.is_nan()
            {
                self.sum -= removed.value;
            }

            if measurement.value.is_finite() {
                self.sum += measurement.value;
            }
        } else {
            self.history.clear();
            if measurement.value.is_nan() {
                self.sum = 0.0;
            } else {
                self.sum = measurement.value;
            }
        }

        self.history.push_back(measurement);
    }
}

// -----------------------------------------------------------------------------
// DiagnosticsStore

/// Global diagnostic registry and measurement storage.
#[derive(Debug, Default, Resource)]
pub struct DiagnosticsStore {
    diagnostics: HashMap<DiagnosticPath, Diagnostic>,
}

impl DiagnosticsStore {
    /// Adds or replaces a diagnostic definition.
    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.insert(diagnostic.path.clone(), diagnostic);
    }

    /// Gets a diagnostic by path.
    pub fn get(&self, path: &DiagnosticPath) -> Option<&Diagnostic> {
        self.diagnostics.get(path)
    }

    /// Mutably gets a diagnostic by path.
    pub fn get_mut(&mut self, path: &DiagnosticPath) -> Option<&mut Diagnostic> {
        self.diagnostics.get_mut(path)
    }

    /// Returns the latest measurement for an enabled diagnostic.
    pub fn get_measurement(&self, path: &DiagnosticPath) -> Option<&DiagnosticMeasurement> {
        self.diagnostics
            .get(path)
            .filter(|diagnostic| diagnostic.is_enabled)
            .and_then(Diagnostic::measurement)
    }

    /// Returns an iterator over diagnostics.
    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.values()
    }

    /// Returns a mutable iterator over diagnostics.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Diagnostic> {
        self.diagnostics.values_mut()
    }

    /// Adds a measurement using an eagerly computed value.
    pub fn add_measurement(&mut self, path: &DiagnosticPath, value: f64) {
        if let Some(diagnostic) = self.diagnostics.get_mut(path)
            && diagnostic.is_enabled
        {
            diagnostic.add_measurement(DiagnosticMeasurement {
                time: Instant::now(),
                value,
            });
        }
    }

    /// Adds a measurement lazily, evaluating `value` only when enabled.
    pub fn add_measurement_with<F>(&mut self, path: &DiagnosticPath, value: F)
    where
        F: FnOnce() -> f64,
    {
        if self
            .diagnostics
            .get(path)
            .is_some_and(|diagnostic| diagnostic.is_enabled)
        {
            self.add_measurement(path, value());
        }
    }
}

// -----------------------------------------------------------------------------
// RegisterDiagnostic

/// Extends app builders with `register_diagnostic`.
pub trait RegisterDiagnostic {
    /// Registers a diagnostic in the app's [`DiagnosticsStore`].
    fn register_diagnostic(&mut self, diagnostic: Diagnostic) -> &mut Self;
}

impl RegisterDiagnostic for SubApp {
    fn register_diagnostic(&mut self, diagnostic: Diagnostic) -> &mut Self {
        self.world_mut()
            .resource_mut_or_init::<DiagnosticsStore>()
            .add(diagnostic);
        self
    }
}

impl RegisterDiagnostic for App {
    fn register_diagnostic(&mut self, diagnostic: Diagnostic) -> &mut Self {
        SubApp::register_diagnostic(self.main_mut(), diagnostic);
        self
    }
}

// -----------------------------------------------------------------------------
// Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clear_history() {
        const MEASUREMENT: f64 = 20.0;

        let mut diagnostic =
            Diagnostic::new(DiagnosticPath::new("test")).with_max_history_length(5);
        let mut now = Instant::now();

        for _ in 0..3 {
            for _ in 0..5 {
                diagnostic.add_measurement(DiagnosticMeasurement {
                    time: now,
                    value: MEASUREMENT,
                });
                now += Duration::from_secs(1);
            }
            assert!((diagnostic.average().expect("average") - MEASUREMENT).abs() < 0.1);
            assert!((diagnostic.smoothed().expect("smoothed") - MEASUREMENT).abs() < 0.1);
            diagnostic.clear_history();
        }
    }
}
