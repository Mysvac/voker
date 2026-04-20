use core::time::Duration;

use log::debug;
use serde::{Deserialize, Serialize};
use voker_reflect::Reflect;

use crate::{Real, Time};

#[derive(Debug, Copy, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Virtual {
    max_delta: Duration,
    paused: bool,
    relative_speed: f64,
    effective_speed: f64,
}

impl Default for Virtual {
    fn default() -> Self {
        Self {
            max_delta: Time::<Virtual>::DEFAULT_MAX_DELTA,
            paused: false,
            relative_speed: 1.0,
            effective_speed: 1.0,
        }
    }
}

impl Time<Virtual> {
    const DEFAULT_MAX_DELTA: Duration = Duration::from_millis(250);

    pub fn from_max_delta(max_delta: Duration) -> Self {
        let mut ret = Self::default();
        ret.set_max_delta(max_delta);
        ret
    }

    #[inline]
    pub fn max_delta(&self) -> Duration {
        self.context().max_delta
    }

    #[inline]
    pub fn set_max_delta(&mut self, max_delta: Duration) {
        assert_ne!(max_delta, Duration::ZERO, "tried to set max delta to zero");
        self.context_mut().max_delta = max_delta;
    }

    #[inline]
    pub fn relative_speed(&self) -> f32 {
        self.relative_speed_f64() as f32
    }

    #[inline]
    pub fn relative_speed_f64(&self) -> f64 {
        self.context().relative_speed
    }

    #[inline]
    pub fn effective_speed(&self) -> f32 {
        self.context().effective_speed as f32
    }

    #[inline]
    pub fn effective_speed_f64(&self) -> f64 {
        self.context().effective_speed
    }

    #[inline]
    pub fn set_relative_speed(&mut self, ratio: f32) {
        self.set_relative_speed_f64(ratio as f64);
    }

    #[inline]
    pub fn set_relative_speed_f64(&mut self, ratio: f64) {
        assert!(ratio.is_finite(), "tried to go infinitely fast");
        assert!(ratio >= 0.0, "tried to go back in time");
        self.context_mut().relative_speed = ratio;
    }

    #[inline]
    pub fn toggle(&mut self) {
        self.context_mut().paused ^= true;
    }

    #[inline]
    pub fn pause(&mut self) {
        self.context_mut().paused = true;
    }

    #[inline]
    pub fn unpause(&mut self) {
        self.context_mut().paused = false;
    }

    #[inline]
    pub fn is_paused(&self) -> bool {
        self.context().paused
    }

    #[inline]
    pub fn was_paused(&self) -> bool {
        self.context().effective_speed == 0.0
    }

    fn advance_with_raw_delta(&mut self, raw_delta: Duration) {
        let max_delta = self.context().max_delta;
        let clamped_delta = if raw_delta > max_delta {
            debug!(
                "delta time larger than maximum delta, clamping delta to {:?} and skipping {:?}",
                max_delta,
                raw_delta - max_delta
            );
            max_delta
        } else {
            raw_delta
        };

        let effective_speed = if self.context().paused {
            0.0
        } else {
            self.context().relative_speed
        };

        let delta = if effective_speed != 1.0 {
            clamped_delta.mul_f64(effective_speed)
        } else {
            clamped_delta
        };

        self.context_mut().effective_speed = effective_speed;
        self.advance_by(delta);
    }
}

pub fn update_virtual_time(current: &mut Time, virt: &mut Time<Virtual>, real: &Time<Real>) {
    let raw_delta = real.delta();
    virt.advance_with_raw_delta(raw_delta);
    *current = virt.as_generic();
}
