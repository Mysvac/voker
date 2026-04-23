use core::time::Duration;

use voker_ecs::borrow::Res;

use crate::{Real, Time, Timer, TimerMode, Virtual};

/// Run condition that is active on a regular time interval,
/// using [`Time`] to advance the timer.
///
/// The timer ticks at the rate of [`Time<Virtual>::relative_speed`][crate::Virtual].
pub fn on_timer(duration: Duration) -> impl FnMut(Res<Time>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Repeating);
    move |time: Res<Time>| {
        timer.tick(time.delta());
        timer.just_finished()
    }
}

/// Run condition that is active on a regular time interval,
/// using [`Time<Real>`] to advance the timer.
///
/// The timer ticks are not scaled.
pub fn on_real_timer(duration: Duration) -> impl FnMut(Res<Time<Real>>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Repeating);
    move |time: Res<Time<Real>>| {
        timer.tick(time.delta());
        timer.just_finished()
    }
}

/// Run condition that is active *once* after the specified delay,
/// using [`Time`] to advance the timer.
///
/// The timer ticks at the rate of [`Time<Virtual>::relative_speed`][crate::Virtual].
pub fn once_after_delay(duration: Duration) -> impl FnMut(Res<Time>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Once);
    move |time: Res<Time>| {
        timer.tick(time.delta());
        timer.just_finished()
    }
}

/// Run condition that is active *once* after the specified delay,
/// using [`Time<Real>`] to advance the timer.
///
/// The timer ticks are not scaled.
pub fn once_after_real_delay(duration: Duration) -> impl FnMut(Res<Time<Real>>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Once);
    move |time: Res<Time<Real>>| {
        timer.tick(time.delta());
        timer.just_finished()
    }
}

/// Run condition that is active *indefinitely* after the specified delay,
/// using [`Time`] to advance the timer.
///
/// The timer ticks at the rate of [`Time<Virtual>::relative_speed`][crate::Virtual].
pub fn repeating_after_delay(duration: Duration) -> impl FnMut(Res<Time>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Once);
    move |time: Res<Time>| {
        timer.tick(time.delta());
        timer.is_finished()
    }
}

/// Run condition that is active *indefinitely* after the specified delay,
/// using [`Time<Real>`] to advance the timer.
///
/// The timer ticks are not scaled.
pub fn repeating_after_real_delay(
    duration: Duration,
) -> impl FnMut(Res<Time<Real>>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Once);
    move |time: Res<Time<Real>>| {
        timer.tick(time.delta());
        timer.is_finished()
    }
}

/// Run condition that is active when the [`Time<Virtual>`] clock is paused.
pub fn paused(time: Res<Time<Virtual>>) -> bool {
    time.is_paused()
}
