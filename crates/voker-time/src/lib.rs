//! Time utilities and scheduling support
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![no_std]

// -----------------------------------------------------------------------------
// no_std support

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

// -----------------------------------------------------------------------------
// Modules

mod fixed;
mod real;
mod stopwatch;
mod time;
mod timer;
mod virt;

// -----------------------------------------------------------------------------
// Exports

pub use fixed::*;
pub use real::*;
pub use stopwatch::*;
pub use time::*;
pub use timer::*;
pub use virt::*;

pub mod conditions;
pub mod delayed;

pub mod prelude {
    #[doc(hidden)]
    pub use crate::{
        DelayedCommandsExt, Fixed, Real, Time, TimePlugin, TimeUpdateStrategy, Timer, TimerMode,
        Virtual,
    };
}

// -----------------------------------------------------------------------------
// Plugin

use core::time::Duration;

use voker_app::{EnableFixedMain, RunFixedMainLoop, prelude::*};
use voker_ecs::borrow::{Res, ResMut};
use voker_ecs::reflect::ReflectResource;
use voker_ecs::resource::Resource;
use voker_ecs::schedule::{IntoSystemConfig, SystemSet};
use voker_os::time::Instant;
use voker_reflect::Reflect;

pub use crate::delayed::DelayedCommandsExt;
use crate::delayed::{DelayedCommandQueues, check_delayed_command_queues};

#[derive(Default)]
pub struct TimePlugin;

/// System set containing the core time update systems added by [`TimePlugin`].
#[derive(Debug, PartialEq, Eq, Clone, Hash, SystemSet)]
pub struct TimeSystems;

/// Configuration resource used to determine how the time system should run.
#[derive(Resource, Default, Reflect, Clone, Debug)]
#[reflect(Default, Clone, Debug)]
#[type_data(ReflectResource)]
pub enum TimeUpdateStrategy {
    #[default]
    Automatic,
    ManualInstant(Instant),
    ManualDuration(Duration),
    FixedTimesteps(u32),
}

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        use RunFixedMainLoopSystems::FixedMainLoop;

        app.init_resource::<Time>()
            .init_resource::<Time<Real>>()
            .init_resource::<Time<Virtual>>()
            .init_resource::<Time<Fixed>>()
            .init_resource::<TimeUpdateStrategy>()
            .init_resource::<EnableFixedMain>()
            .init_resource::<DelayedCommandQueues>();

        app.register_type::<TimeUpdateStrategy>();

        app.add_systems(First, time_system.in_set(TimeSystems));
        app.add_systems(PreUpdate, check_delayed_command_queues);
        app.add_systems(
            RunFixedMainLoop,
            run_fixed_main_schedule.in_set(FixedMainLoop),
        );
    }
}

/// Reads the current [`TimeUpdateStrategy`] and advances the real, virtual, and fixed clocks.
pub fn time_system(
    mut real_time: ResMut<Time<Real>>,
    mut virtual_time: ResMut<Time<Virtual>>,
    fixed_time: Res<Time<Fixed>>,
    mut time: ResMut<Time>,
    update_strategy: Res<TimeUpdateStrategy>,
) {
    match update_strategy.as_ref() {
        TimeUpdateStrategy::Automatic => real_time.update_with_instant(Instant::now()),
        TimeUpdateStrategy::ManualInstant(instant) => real_time.update_with_instant(*instant),
        TimeUpdateStrategy::ManualDuration(duration) => real_time.update_with_duration(*duration),
        TimeUpdateStrategy::FixedTimesteps(factor) => {
            real_time.update_with_duration(*factor * fixed_time.timestep());
        }
    }

    update_virtual_time(&mut time, &mut virtual_time, &real_time);
}
