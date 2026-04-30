use core::time::Duration;

use serde::{Deserialize, Serialize};
use voker_app::FixedMain;
use voker_ecs::world::World;
use voker_reflect::Reflect;

use crate::{Time, Virtual};

// -----------------------------------------------------------------------------
// Fixed

/// Context for fixed-timestep time, tracking the step duration and accumulated overstep.
#[derive(Debug, Copy, Clone, Reflect, Serialize, Deserialize)]
#[reflect(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Fixed {
    timestep: Duration,
    overstep: Duration,
}

impl Default for Fixed {
    fn default() -> Self {
        Self {
            timestep: Time::<Fixed>::DEFAULT_TIMESTEP,
            overstep: Duration::ZERO,
        }
    }
}

impl Time<Fixed> {
    const DEFAULT_TIMESTEP: Duration = Duration::from_micros(15625);

    /// Creates a `Time<Fixed>` with the given timestep duration.
    pub fn from_duration(timestep: Duration) -> Self {
        let mut ret = Self::default();
        ret.set_timestep(timestep);
        ret
    }

    /// Creates a `Time<Fixed>` with a timestep of `seconds` seconds.
    pub fn from_seconds(seconds: f64) -> Self {
        let mut ret = Self::default();
        ret.set_timestep_seconds(seconds);
        ret
    }

    /// Creates a `Time<Fixed>` with a timestep matching the given frequency in Hz.
    pub fn from_hz(hz: f64) -> Self {
        let mut ret = Self::default();
        ret.set_timestep_hz(hz);
        ret
    }

    /// Returns the fixed timestep duration.
    #[inline]
    pub fn timestep(&self) -> Duration {
        self.context().timestep
    }

    /// Sets the fixed timestep duration; panics if zero.
    #[inline]
    pub fn set_timestep(&mut self, timestep: Duration) {
        assert_ne!(
            timestep,
            Duration::ZERO,
            "attempted to set fixed timestep to zero"
        );
        self.context_mut().timestep = timestep;
    }

    /// Sets the fixed timestep from seconds; panics if not positive or not finite.
    #[inline]
    pub fn set_timestep_seconds(&mut self, seconds: f64) {
        assert!(seconds > 0.0, "seconds must be positive and non-zero");
        assert!(seconds.is_finite(), "seconds is infinite");
        self.set_timestep(Duration::from_secs_f64(seconds));
    }

    /// Sets the fixed timestep from a frequency in Hz; panics if not positive or not finite.
    #[inline]
    pub fn set_timestep_hz(&mut self, hz: f64) {
        assert!(hz > 0.0, "Hz must be positive and non-zero");
        assert!(hz.is_finite(), "Hz is infinite");
        self.set_timestep_seconds(1.0 / hz);
    }

    /// Returns the accumulated time beyond the last consumed timestep.
    #[inline]
    pub fn overstep(&self) -> Duration {
        self.context().overstep
    }

    /// Adds `delta` to the overstep accumulator.
    #[inline]
    pub fn accumulate_overstep(&mut self, delta: Duration) {
        self.context_mut().overstep += delta;
    }

    /// Saturating-subtracts `discard` from the overstep accumulator.
    #[inline]
    pub fn discard_overstep(&mut self, discard: Duration) {
        let context = self.context_mut();
        context.overstep = context.overstep.saturating_sub(discard);
    }

    /// Returns the overstep as a fraction of the timestep, as `f32`.
    #[inline]
    pub fn overstep_fraction(&self) -> f32 {
        self.context().overstep.as_secs_f32() / self.context().timestep.as_secs_f32()
    }

    /// Returns the overstep as a fraction of the timestep, as `f64`.
    #[inline]
    pub fn overstep_fraction_f64(&self) -> f64 {
        self.context().overstep.as_secs_f64() / self.context().timestep.as_secs_f64()
    }

    fn expend(&mut self) -> bool {
        let timestep = self.timestep();
        if let Some(new_value) = self.context_mut().overstep.checked_sub(timestep) {
            self.context_mut().overstep = new_value;
            self.advance_by(timestep);
            true
        } else {
            false
        }
    }
}

/// Runs the `FixedMain` schedule as many times as the virtual-time overstep allows.
pub fn run_fixed_main_schedule(world: &mut World) {
    let delta = world.resource::<Time<Virtual>>().delta();
    world.resource_mut::<Time<Fixed>>().accumulate_overstep(delta);

    let _ = world.try_schedule_scope(FixedMain, |world, schedule| {
        while world.resource_mut::<Time<Fixed>>().expend() {
            *world.resource_mut::<Time>() = world.resource::<Time<Fixed>>().as_generic();
            schedule.run(world);
        }
    });

    *world.resource_mut::<Time>() = world.resource::<Time<Virtual>>().as_generic();
}
