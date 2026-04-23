use core::time::Duration;

use serde::{Deserialize, Serialize};
use voker_reflect::Reflect;

use crate::Stopwatch;

/// Controls whether a [`Timer`] repeats after finishing.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[derive(Reflect, Default, Serialize, Deserialize)]
#[reflect(Default, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub enum TimerMode {
    /// The timer runs once and stays in the finished state.
    #[default]
    Once,
    /// The timer resets from zero each time it finishes.
    Repeating,
}

/// A countdown timer that tracks elapsed time against a target duration.
#[derive(Reflect, Serialize, Deserialize)]
#[derive(Debug, Clone, Eq, PartialEq, Default)]
#[reflect(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    stopwatch: Stopwatch,
    duration: Duration,
    mode: TimerMode,
    finished: bool,
    times_finished_this_tick: u32,
}

impl Timer {
    /// Creates a new timer for the given `duration` and `mode`.
    pub fn new(duration: Duration, mode: TimerMode) -> Self {
        Self {
            duration,
            mode,
            ..Default::default()
        }
    }

    /// Creates a new timer with a duration of `duration` seconds.
    pub fn from_seconds(duration: f32, mode: TimerMode) -> Self {
        Self {
            duration: Duration::from_secs_f32(duration),
            mode,
            ..Default::default()
        }
    }

    /// Returns `true` if the timer has completed at least once.
    #[inline]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Returns `true` if the timer finished during the current tick.
    #[inline]
    pub fn just_finished(&self) -> bool {
        self.times_finished_this_tick > 0
    }

    /// Returns the time elapsed since the timer last reset or started.
    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.stopwatch.elapsed()
    }

    /// Returns [`elapsed`][Self::elapsed] as `f32` seconds.
    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.stopwatch.elapsed_secs()
    }

    /// Returns [`elapsed`][Self::elapsed] as `f64` seconds.
    #[inline]
    pub fn elapsed_secs_f64(&self) -> f64 {
        self.stopwatch.elapsed_secs_f64()
    }

    /// Sets the elapsed time directly, bypassing normal ticking.
    #[inline]
    pub fn set_elapsed(&mut self, time: Duration) {
        self.stopwatch.set_elapsed(time);
    }

    /// Returns the target duration of this timer.
    #[inline]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Sets the target duration.
    #[inline]
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    /// Returns the time remaining until the timer finishes.
    #[inline]
    pub fn remaining(&self) -> Duration {
        self.duration().saturating_sub(self.elapsed())
    }

    /// Returns [`remaining`][Self::remaining] as `f32` seconds.
    #[inline]
    pub fn remaining_secs(&self) -> f32 {
        self.remaining().as_secs_f32()
    }

    /// Advances the timer to exactly finished in the current tick.
    #[inline]
    pub fn finish(&mut self) {
        let remaining = self.remaining();
        self.tick(remaining);
    }

    /// Advances the timer to one nanosecond before finished; no-op if already finished.
    #[inline]
    pub fn almost_finish(&mut self) {
        let remaining = self.remaining().saturating_sub(Duration::from_nanos(1));
        self.tick(remaining);
    }

    /// Returns the current [`TimerMode`].
    #[inline]
    pub fn mode(&self) -> TimerMode {
        self.mode
    }

    /// Sets the [`TimerMode`], resetting state if switching from `Once` to `Repeating` while finished.
    #[inline]
    pub fn set_mode(&mut self, mode: TimerMode) {
        if self.mode != TimerMode::Repeating && mode == TimerMode::Repeating && self.finished {
            self.stopwatch.reset();
            self.finished = self.just_finished();
        }
        self.mode = mode;
    }

    /// Pauses the underlying stopwatch so the timer does not advance.
    #[inline]
    pub fn pause(&mut self) {
        self.stopwatch.pause();
    }

    /// Resumes the underlying stopwatch.
    #[inline]
    pub fn unpause(&mut self) {
        self.stopwatch.unpause();
    }

    /// Returns `true` if the timer is currently paused.
    #[inline]
    pub fn is_paused(&self) -> bool {
        self.stopwatch.is_paused()
    }

    /// Resets elapsed time and clears the finished state.
    #[inline]
    pub fn reset(&mut self) {
        self.stopwatch.reset();
        self.finished = false;
        self.times_finished_this_tick = 0;
    }

    /// Returns the completion progress as a value in `[0.0, 1.0]`.
    #[inline]
    pub fn fraction(&self) -> f32 {
        if self.duration == Duration::ZERO {
            1.0
        } else {
            self.elapsed().as_secs_f32() / self.duration().as_secs_f32()
        }
    }

    /// Returns the remaining fraction as `1.0 - fraction()`.
    #[inline]
    pub fn fraction_remaining(&self) -> f32 {
        1.0 - self.fraction()
    }

    /// Returns how many times the timer finished during the current tick.
    #[inline]
    pub fn times_finished_this_tick(&self) -> u32 {
        self.times_finished_this_tick
    }

    /// Advances the timer by `delta`, updating finished state and repeat tracking.
    pub fn tick(&mut self, delta: Duration) -> &Self {
        self.times_finished_this_tick = 0;

        if self.is_paused() {
            if self.mode == TimerMode::Repeating {
                self.finished = false;
            }
            return self;
        }

        if self.mode != TimerMode::Repeating && self.is_finished() {
            return self;
        }

        self.stopwatch.tick(delta);
        self.finished = self.elapsed() >= self.duration();

        if self.is_finished() {
            if self.mode == TimerMode::Repeating {
                self.times_finished_this_tick = self
                    .elapsed()
                    .as_nanos()
                    .checked_div(self.duration().as_nanos())
                    .map_or(u32::MAX, |x| x as u32);

                let elapsed = self
                    .elapsed()
                    .as_nanos()
                    .checked_rem(self.duration().as_nanos())
                    .map_or(Duration::ZERO, |x| Duration::from_nanos(x as u64));
                self.set_elapsed(elapsed);
            } else {
                self.times_finished_this_tick = 1;
                self.set_elapsed(self.duration());
            }
        }

        self
    }
}
