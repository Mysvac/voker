use core::time::Duration;

use serde::{Deserialize, Serialize};
use voker_app::FixedMain;
use voker_ecs::world::World;
use voker_reflect::Reflect;

use crate::{Time, Virtual};

// -----------------------------------------------------------------------------
// Fixed

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

    pub fn from_duration(timestep: Duration) -> Self {
        let mut ret = Self::default();
        ret.set_timestep(timestep);
        ret
    }

    pub fn from_seconds(seconds: f64) -> Self {
        let mut ret = Self::default();
        ret.set_timestep_seconds(seconds);
        ret
    }

    pub fn from_hz(hz: f64) -> Self {
        let mut ret = Self::default();
        ret.set_timestep_hz(hz);
        ret
    }

    #[inline]
    pub fn timestep(&self) -> Duration {
        self.context().timestep
    }

    #[inline]
    pub fn set_timestep(&mut self, timestep: Duration) {
        assert_ne!(
            timestep,
            Duration::ZERO,
            "attempted to set fixed timestep to zero"
        );
        self.context_mut().timestep = timestep;
    }

    #[inline]
    pub fn set_timestep_seconds(&mut self, seconds: f64) {
        assert!(
            seconds.is_sign_positive(),
            "seconds less than or equal to zero"
        );
        assert!(seconds.is_finite(), "seconds is infinite");
        self.set_timestep(Duration::from_secs_f64(seconds));
    }

    #[inline]
    pub fn set_timestep_hz(&mut self, hz: f64) {
        assert!(hz.is_sign_positive(), "Hz less than or equal to zero");
        assert!(hz.is_finite(), "Hz is infinite");
        self.set_timestep_seconds(1.0 / hz);
    }

    #[inline]
    pub fn overstep(&self) -> Duration {
        self.context().overstep
    }

    #[inline]
    pub fn accumulate_overstep(&mut self, delta: Duration) {
        self.context_mut().overstep += delta;
    }

    #[inline]
    pub fn discard_overstep(&mut self, discard: Duration) {
        let context = self.context_mut();
        context.overstep = context.overstep.saturating_sub(discard);
    }

    #[inline]
    pub fn overstep_fraction(&self) -> f32 {
        self.context().overstep.as_secs_f32() / self.context().timestep.as_secs_f32()
    }

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
