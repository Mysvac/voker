use core::time::Duration;

use voker_os::time::Instant;
use voker_reflect::Reflect;

use crate::time::Time;

/// Context for wall-clock time, tracking startup and update instants.
#[derive(Debug, Copy, Clone, Reflect)]
#[reflect(Debug, Clone, Default)]
pub struct Real {
    startup: Instant,
    first_update: Option<Instant>,
    last_update: Option<Instant>,
}

impl Default for Real {
    fn default() -> Self {
        Self {
            startup: Instant::now(),
            first_update: None,
            last_update: None,
        }
    }
}

impl Time<Real> {
    /// Creates a new `Time<Real>` with the given `startup` instant.
    pub fn new(startup: Instant) -> Self {
        Self::new_with(Real {
            startup,
            ..Default::default()
        })
    }

    /// Advances real time using `Instant::now()`.
    pub fn update(&mut self) {
        self.update_with_instant(Instant::now());
    }

    /// Advances real time by a fixed `duration` from the last update instant.
    pub fn update_with_duration(&mut self, duration: Duration) {
        let last_update = self.context().last_update.unwrap_or(self.context().startup);
        self.update_with_instant(last_update + duration);
    }

    /// Advances real time to the given `instant`, computing delta since the last update.
    pub fn update_with_instant(&mut self, instant: Instant) {
        let Some(last_update) = self.context().last_update else {
            let context = self.context_mut();
            context.first_update = Some(instant);
            context.last_update = Some(instant);
            return;
        };

        let delta = instant.saturating_duration_since(last_update);
        self.advance_by(delta);
        self.context_mut().last_update = Some(instant);
    }

    /// Returns the `Instant` at which this clock was created.
    #[inline]
    pub fn startup(&self) -> Instant {
        self.context().startup
    }

    /// Returns the `Instant` of the first call to any update method, or `None` if not yet updated.
    #[inline]
    pub fn first_update(&self) -> Option<Instant> {
        self.context().first_update
    }

    /// Returns the `Instant` of the most recent update, or `None` if not yet updated.
    #[inline]
    pub fn last_update(&self) -> Option<Instant> {
        self.context().last_update
    }
}
