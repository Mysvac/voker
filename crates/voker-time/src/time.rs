use core::time::Duration;

use serde::{Deserialize, Serialize};
use voker_ecs::resource::Resource;
use voker_reflect::Reflect;

#[derive(Resource, Debug, Copy, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Resource, Default)]
pub struct Time<T: Default = ()> {
    context: T,
    wrap_period: Duration,
    delta: Duration,
    delta_secs: f32,
    delta_secs_f64: f64,
    elapsed: Duration,
    elapsed_secs: f32,
    elapsed_secs_f64: f64,
    elapsed_wrapped: Duration,
    elapsed_secs_wrapped: f32,
    elapsed_secs_wrapped_f64: f64,
}

impl<T: Default> Time<T> {
    const DEFAULT_WRAP_PERIOD: Duration = Duration::from_secs(3600);

    pub fn new_with(context: T) -> Self {
        Self {
            context,
            ..Default::default()
        }
    }

    pub fn advance_by(&mut self, delta: Duration) {
        self.delta = delta;
        self.delta_secs = self.delta.as_secs_f32();
        self.delta_secs_f64 = self.delta.as_secs_f64();
        self.elapsed += delta;
        self.elapsed_secs = self.elapsed.as_secs_f32();
        self.elapsed_secs_f64 = self.elapsed.as_secs_f64();
        self.elapsed_wrapped = duration_rem(self.elapsed, self.wrap_period);
        self.elapsed_secs_wrapped = self.elapsed_wrapped.as_secs_f32();
        self.elapsed_secs_wrapped_f64 = self.elapsed_wrapped.as_secs_f64();
    }

    pub fn advance_to(&mut self, elapsed: Duration) {
        assert!(
            elapsed >= self.elapsed,
            "tried to move time backwards to an earlier elapsed moment"
        );
        self.advance_by(elapsed - self.elapsed);
    }

    #[inline]
    pub fn wrap_period(&self) -> Duration {
        self.wrap_period
    }

    #[inline]
    pub fn set_wrap_period(&mut self, wrap_period: Duration) {
        assert!(!wrap_period.is_zero(), "division by zero");
        self.wrap_period = wrap_period;
    }

    #[inline]
    pub fn delta(&self) -> Duration {
        self.delta
    }

    #[inline]
    pub fn delta_secs(&self) -> f32 {
        self.delta_secs
    }

    #[inline]
    pub fn delta_secs_f64(&self) -> f64 {
        self.delta_secs_f64
    }

    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    #[inline]
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed_secs
    }

    #[inline]
    pub fn elapsed_secs_f64(&self) -> f64 {
        self.elapsed_secs_f64
    }

    #[inline]
    pub fn elapsed_wrapped(&self) -> Duration {
        self.elapsed_wrapped
    }

    #[inline]
    pub fn elapsed_secs_wrapped(&self) -> f32 {
        self.elapsed_secs_wrapped
    }

    #[inline]
    pub fn elapsed_secs_wrapped_f64(&self) -> f64 {
        self.elapsed_secs_wrapped_f64
    }

    #[inline]
    pub fn context(&self) -> &T {
        &self.context
    }

    #[inline]
    pub fn context_mut(&mut self) -> &mut T {
        &mut self.context
    }

    #[inline]
    pub fn as_generic(&self) -> Time<()> {
        Time {
            context: (),
            wrap_period: self.wrap_period,
            delta: self.delta,
            delta_secs: self.delta_secs,
            delta_secs_f64: self.delta_secs_f64,
            elapsed: self.elapsed,
            elapsed_secs: self.elapsed_secs,
            elapsed_secs_f64: self.elapsed_secs_f64,
            elapsed_wrapped: self.elapsed_wrapped,
            elapsed_secs_wrapped: self.elapsed_secs_wrapped,
            elapsed_secs_wrapped_f64: self.elapsed_secs_wrapped_f64,
        }
    }
}

impl<T: Default> Default for Time<T> {
    fn default() -> Self {
        Self {
            context: Default::default(),
            wrap_period: Self::DEFAULT_WRAP_PERIOD,
            delta: Duration::ZERO,
            delta_secs: 0.0,
            delta_secs_f64: 0.0,
            elapsed: Duration::ZERO,
            elapsed_secs: 0.0,
            elapsed_secs_f64: 0.0,
            elapsed_wrapped: Duration::ZERO,
            elapsed_secs_wrapped: 0.0,
            elapsed_secs_wrapped_f64: 0.0,
        }
    }
}

fn duration_rem(dividend: Duration, divisor: Duration) -> Duration {
    let quotient = (dividend.as_nanos() / divisor.as_nanos()) as u32;
    dividend - (quotient * divisor)
}
