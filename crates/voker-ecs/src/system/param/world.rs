use super::SystemParam;
use crate::error::EcsError;
use crate::system::AccessTable;
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

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
    ) -> Result<Self::Item<'w, 's>, EcsError> {
        unsafe { Ok(world.read_only()) }
    }
}

unsafe impl SystemParam for &mut World {
    type State = ();
    type Item<'world, 'state> = &'world mut World;
    // `&mut World` is treated as `NonSend` because mutable world
    // access may include operations on `NonSend` resources.
    const NON_SEND: bool = true;
    // We hold the mutable borrowing of this world,
    // so we cannot parallelize with other systems.
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
    ) -> Result<Self::Item<'w, 's>, EcsError> {
        unsafe { Ok(world.full_mut()) }
    }
}
