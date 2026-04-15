use core::marker::PhantomData;

use super::SystemParam;
use crate::system::{AccessTable, SystemParamError};
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

unsafe impl<T> SystemParam for PhantomData<T> {
    type State = ();
    type Item<'world, 'state> = PhantomData<T>;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(_world: &mut World) -> Self::State {}

    fn mark_access(_table: &mut AccessTable, _state: &Self::State) -> bool {
        true
    }

    unsafe fn build_param<'w, 's>(
        _world: UnsafeWorld<'w>,
        _state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        Ok(PhantomData)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NonSendMarker;

unsafe impl SystemParam for NonSendMarker {
    type State = ();
    type Item<'world, 'state> = NonSendMarker;

    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = false;

    fn init_state(_world: &mut World) -> Self::State {}

    fn mark_access(_table: &mut AccessTable, _state: &Self::State) -> bool {
        true
    }

    unsafe fn build_param<'w, 's>(
        _world: UnsafeWorld<'w>,
        _state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        Ok(NonSendMarker)
    }
}
