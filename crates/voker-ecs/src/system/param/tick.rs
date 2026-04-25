use crate::system::{AccessTable, SystemParam, SystemParamError};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

#[derive(Debug, Clone, Copy)]
pub struct SystemTick {
    last_run: Tick,
    this_run: Tick,
}

impl SystemTick {
    /// Returns the current [`World`] change tick seen by the system.
    #[inline]
    pub fn this_run(&self) -> Tick {
        self.this_run
    }

    /// Returns the [`World`] change tick seen by the system the previous time it ran.
    #[inline]
    pub fn last_run(&self) -> Tick {
        self.last_run
    }
}

// SAFETY: `SystemTick` doesn't require any world access
unsafe impl SystemParam for SystemTick {
    type State = ();
    type Item<'world, 'state> = SystemTick;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(_world: &mut World) -> Self::State {}

    fn mark_access(_table: &mut AccessTable, _state: &Self::State) -> bool {
        true
    }

    unsafe fn build_param<'w, 's>(
        _world: UnsafeWorld<'w>,
        _state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        Ok(SystemTick { last_run, this_run })
    }
}
