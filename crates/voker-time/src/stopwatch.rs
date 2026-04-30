use core::time::Duration;

use serde::{Deserialize, Serialize};
use voker_reflect::Reflect;

/// A simple stopwatch that tracks accumulated elapsed time and can be paused.
#[derive(Reflect, Serialize, Deserialize)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stopwatch {
    elapsed: Duration,
    is_paused: bool,
}

impl Stopwatch {
    /// Creates a new, unpaused stopwatch at zero elapsed time.
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// Returns the total elapsed time.
    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    /// Returns [`elapsed`][Self::elapsed] as `f32` seconds.
    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed().as_secs_f32()
    }

    /// Returns [`elapsed`][Self::elapsed] as `f64` seconds.
    #[inline]
    pub fn elapsed_secs_f64(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }

    /// Sets the elapsed time directly.
    #[inline]
    pub fn set_elapsed(&mut self, time: Duration) {
        self.elapsed = time;
    }

    /// Pauses the stopwatch; subsequent ticks will not advance elapsed time.
    #[inline]
    pub fn pause(&mut self) {
        self.is_paused = true;
    }

    /// Resumes the stopwatch.
    #[inline]
    pub fn unpause(&mut self) {
        self.is_paused = false;
    }

    /// Returns `true` if the stopwatch is currently paused.
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    /// Resets elapsed time to zero without changing the pause state.
    #[inline]
    pub fn reset(&mut self) {
        self.elapsed = Default::default();
    }

    /// Advances elapsed time by `delta` unless paused; returns `&Self`.
    #[inline]
    pub fn tick(&mut self, delta: Duration) -> &Self {
        if !self.is_paused {
            self.elapsed = self.elapsed.saturating_add(delta);
        }
        self
    }
}
