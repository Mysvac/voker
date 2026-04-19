use core::time::Duration;

use voker_ecs::borrow::Res;

use crate::{Real, Time, Timer, TimerMode, Virtual};

pub fn on_timer(duration: Duration) -> impl FnMut(Res<Time>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Repeating);
    move |time: Res<Time>| {
        timer.tick(time.delta());
        timer.just_finished()
    }
}

pub fn on_real_timer(duration: Duration) -> impl FnMut(Res<Time<Real>>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Repeating);
    move |time: Res<Time<Real>>| {
        timer.tick(time.delta());
        timer.just_finished()
    }
}

pub fn once_after_delay(duration: Duration) -> impl FnMut(Res<Time>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Once);
    move |time: Res<Time>| {
        timer.tick(time.delta());
        timer.just_finished()
    }
}

pub fn once_after_real_delay(duration: Duration) -> impl FnMut(Res<Time<Real>>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Once);
    move |time: Res<Time<Real>>| {
        timer.tick(time.delta());
        timer.just_finished()
    }
}

pub fn repeating_after_delay(duration: Duration) -> impl FnMut(Res<Time>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Once);
    move |time: Res<Time>| {
        timer.tick(time.delta());
        timer.is_finished()
    }
}

pub fn repeating_after_real_delay(
    duration: Duration,
) -> impl FnMut(Res<Time<Real>>) -> bool + Clone {
    let mut timer = Timer::new(duration, TimerMode::Once);
    move |time: Res<Time<Real>>| {
        timer.tick(time.delta());
        timer.is_finished()
    }
}

pub fn paused(time: Res<Time<Virtual>>) -> bool {
    time.is_paused()
}
