use core::time::Duration;

use serde::{Deserialize, Serialize};
use voker_reflect::Reflect;

use crate::Stopwatch;




#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[derive(Reflect, Default, Serialize, Deserialize)]
#[reflect(Default, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub enum TimerMode {
    #[default]
    Once,
    Repeating,
}

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
    pub fn new(duration: Duration, mode: TimerMode) -> Self {
        Self {
            duration,
            mode,
            ..Default::default()
        }
    }

    pub fn from_seconds(duration: f32, mode: TimerMode) -> Self {
        Self {
            duration: Duration::from_secs_f32(duration),
            mode,
            ..Default::default()
        }
    }

    #[inline]
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    #[inline]
    pub fn just_finished(&self) -> bool {
        self.times_finished_this_tick > 0
    }

    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.stopwatch.elapsed()
    }

    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.stopwatch.elapsed_secs()
    }

    #[inline]
    pub fn elapsed_secs_f64(&self) -> f64 {
        self.stopwatch.elapsed_secs_f64()
    }

    #[inline]
    pub fn set_elapsed(&mut self, time: Duration) {
        self.stopwatch.set_elapsed(time);
    }

    #[inline]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    #[inline]
    pub fn set_duration(&mut self, duration: Duration) {
        self.duration = duration;
    }

    #[inline]
    pub fn remaining(&self) -> Duration {
        self.duration().saturating_sub(self.elapsed())
    }

    #[inline]
    pub fn remaining_secs(&self) -> f32 {
        self.remaining().as_secs_f32()
    }

    #[inline]
    pub fn finish(&mut self) {
        let remaining = self.remaining();
        self.tick(remaining);
    }

    #[inline]
    pub fn almost_finish(&mut self) {
        let remaining = self.remaining() - Duration::from_nanos(1);
        self.tick(remaining);
    }

    #[inline]
    pub fn mode(&self) -> TimerMode {
        self.mode
    }

    #[inline]
    pub fn set_mode(&mut self, mode: TimerMode) {
        if self.mode != TimerMode::Repeating && mode == TimerMode::Repeating && self.finished {
            self.stopwatch.reset();
            self.finished = self.just_finished();
        }
        self.mode = mode;
    }

    #[inline]
    pub fn pause(&mut self) {
        self.stopwatch.pause();
    }

    #[inline]
    pub fn unpause(&mut self) {
        self.stopwatch.unpause();
    }

    #[inline]
    pub fn is_paused(&self) -> bool {
        self.stopwatch.is_paused()
    }

    #[inline]
    pub fn reset(&mut self) {
        self.stopwatch.reset();
        self.finished = false;
        self.times_finished_this_tick = 0;
    }

    #[inline]
    pub fn fraction(&self) -> f32 {
        if self.duration == Duration::ZERO {
            1.0
        } else {
            self.elapsed().as_secs_f32() / self.duration().as_secs_f32()
        }
    }

    #[inline]
    pub fn fraction_remaining(&self) -> f32 {
        1.0 - self.fraction()
    }

    #[inline]
    pub fn times_finished_this_tick(&self) -> u32 {
        self.times_finished_this_tick
    }


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

                let elapsed = self.elapsed()
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


