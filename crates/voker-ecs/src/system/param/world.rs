use super::{SystemParam, SystemParamError};
use crate::system::{AccessTable, SystemMeta};
use crate::tick::Tick;
use crate::world::{DeferredWorld, UnsafeWorld, World};

// ---------------------------------------------------------
// World

unsafe impl SystemParam for &World {
    type State = ();
    type Item<'world, 'state> = &'world World;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(_world: &mut World) -> Self::State {}

    fn mark_access(table: &mut AccessTable, _state: &Self::State) -> bool {
        table.set_world_ref()
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        _state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe { Ok(world.read_only()) }
    }
}

unsafe impl SystemParam for &mut World {
    type State = ();
    type Item<'world, 'state> = &'world mut World;

    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = true;

    fn init_state(_world: &mut World) -> Self::State {}

    fn mark_access(table: &mut AccessTable, _state: &Self::State) -> bool {
        table.set_world_mut()
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        _state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe { Ok(world.full_mut()) }
    }
}

unsafe impl SystemParam for DeferredWorld<'_> {
    type State = ();
    type Item<'world, 'state> = DeferredWorld<'world>;

    const DEFERRED: bool = true;
    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = false;

    fn init_state(_world: &mut World) -> Self::State {}

    fn mark_access(table: &mut AccessTable, _state: &Self::State) -> bool {
        table.set_world_mut()
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        _state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe { Ok(world.deferred()) }
    }

    fn apply_deferred(_: &mut Self::State, _: &SystemMeta, world: &mut World) {
        world.flush();
    }
}
