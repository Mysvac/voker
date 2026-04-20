use core::time::Duration;

use serde::{Deserialize, Serialize};
use voker_reflect::Reflect;

#[derive(Reflect, Serialize, Deserialize)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stopwatch {
    elapsed: Duration,
    is_paused: bool,
}

impl Stopwatch {
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed().as_secs_f32()
    }

    #[inline]
    pub fn elapsed_secs_f64(&self) -> f64 {
        self.elapsed().as_secs_f64()
    }

    #[inline]
    pub fn set_elapsed(&mut self, time: Duration) {
        self.elapsed = time;
    }

    #[inline]
    pub fn pause(&mut self) {
        self.is_paused = true;
    }

    #[inline]
    pub fn unpause(&mut self) {
        self.is_paused = false;
    }

    #[inline]
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    #[inline]
    pub fn reset(&mut self) {
        self.elapsed = Default::default();
    }

    #[inline]
    pub fn tick(&mut self, delta: Duration) -> &Self {
        if !self.is_paused {
            self.elapsed = self.elapsed.saturating_add(delta);
        }
        self
    }
}
