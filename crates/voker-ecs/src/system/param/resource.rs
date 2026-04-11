use super::{ReadOnlySystemParam, SystemParam};
use crate::borrow::{NonSend, NonSendMut, NonSendRef};
use crate::borrow::{Res, ResMut, ResRef};
use crate::error::Severity;
use crate::resource::{Resource, ResourceId};
use crate::system::{AccessTable, SystemParamError};
use crate::tick::Tick;
use crate::utils::DebugName;
use crate::world::{UnsafeWorld, World};

#[cold]
#[inline(never)]
fn uninit_resource_error<P, R>() -> SystemParamError {
    SystemParamError::new::<P>()
        .with_severity(Severity::Warning)
        .with_info(alloc::format!(
            "Try to fetch a uninitialized resource `{}`",
            DebugName::type_name::<R>()
        ))
}

// -----------------------------------------------------------------------------
// Res

unsafe impl<T: Resource + Sync> ReadOnlySystemParam for Res<'_, T> {}

unsafe impl<T: Resource + Sync> SystemParam for Res<'_, T> {
    type State = ResourceId;
    type Item<'world, 'state> = Res<'world, T>;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_reading_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.read_only();
            if let Some(data) = world.storages.res_set.get(*state)
                && let Some(ptr) = data.get_data()
            {
                ptr.debug_assert_aligned::<T>();
                Ok(Res {
                    value: ptr.deref::<T>(),
                })
            } else {
                Err(uninit_resource_error::<Self, T>())
            }
        }
    }
}

// -----------------------------------------------------------------------------
// ResRef

unsafe impl<T: Resource + Sync> ReadOnlySystemParam for ResRef<'_, T> {}

unsafe impl<T: Resource + Sync> SystemParam for ResRef<'_, T> {
    type State = ResourceId;
    type Item<'world, 'state> = ResRef<'world, T>;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_reading_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.read_only();
            if let Some(data) = world.storages.res_set.get(*state)
                && let Some(untyped) = data.get_ref(last_run, this_run)
            {
                Ok(untyped.into_resource::<T>())
            } else {
                Err(uninit_resource_error::<Self, T>())
            }
        }
    }
}

// -----------------------------------------------------------------------------
// ResMut

unsafe impl<T: Resource + Send> SystemParam for ResMut<'_, T> {
    type State = ResourceId;
    type Item<'world, 'state> = ResMut<'world, T>;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_writing_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.data_mut();
            if let Some(data) = world.storages.res_set.get_mut(*state)
                && let Some(untyped) = data.get_mut(last_run, this_run)
            {
                Ok(untyped.into_resource::<T>())
            } else {
                Err(uninit_resource_error::<Self, T>())
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Option<Res>

unsafe impl<T: Resource + Sync> ReadOnlySystemParam for Option<Res<'_, T>> {}

unsafe impl<T: Resource + Sync> SystemParam for Option<Res<'_, T>> {
    type State = ResourceId;
    type Item<'world, 'state> = Option<Res<'world, T>>;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_reading_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.read_only();
            let Some(data) = world.storages.res_set.get(*state) else {
                return Ok(None);
            };
            let Some(ptr) = data.get_data() else {
                return Ok(None);
            };
            ptr.debug_assert_aligned::<T>();
            Ok(Some(Res {
                value: ptr.deref::<T>(),
            }))
        }
    }
}

// -----------------------------------------------------------------------------
// Option<ResRef>

unsafe impl<T: Resource + Sync> ReadOnlySystemParam for Option<ResRef<'_, T>> {}

unsafe impl<T: Resource + Sync> SystemParam for Option<ResRef<'_, T>> {
    type State = ResourceId;
    type Item<'world, 'state> = Option<ResRef<'world, T>>;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_reading_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.read_only();
            let Some(data) = world.storages.res_set.get(*state) else {
                return Ok(None);
            };
            let Some(untyped) = data.get_ref(last_run, this_run) else {
                return Ok(None);
            };
            Ok(Some(untyped.into_resource::<T>()))
        }
    }
}

// -----------------------------------------------------------------------------
// Option<ResMut>

unsafe impl<T: Resource + Send> SystemParam for Option<ResMut<'_, T>> {
    type State = ResourceId;
    type Item<'world, 'state> = Option<ResMut<'world, T>>;
    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_writing_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.data_mut();
            let Some(data) = world.storages.res_set.get_mut(*state) else {
                return Ok(None);
            };
            let Some(untyped) = data.get_mut(last_run, this_run) else {
                return Ok(None);
            };
            Ok(Some(untyped.into_resource::<T>()))
        }
    }
}

// -----------------------------------------------------------------------------
// NonSend

unsafe impl<T: Resource> ReadOnlySystemParam for NonSend<'_, T> {}

unsafe impl<T: Resource> SystemParam for NonSend<'_, T> {
    type State = ResourceId;
    type Item<'world, 'state> = NonSend<'world, T>;
    // Because the resource is !Sync, we can only borrow it
    // on the main thread. In other words, this system is !Send.
    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_reading_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.read_only();
            if let Some(data) = world.storages.res_set.get(*state)
                && let Some(ptr) = data.get_data()
            {
                ptr.debug_assert_aligned::<T>();
                Ok(NonSend {
                    value: ptr.deref::<T>(),
                })
            } else {
                Err(uninit_resource_error::<Self, T>())
            }
        }
    }
}

// -----------------------------------------------------------------------------
// NonSendRef

unsafe impl<T: Resource> ReadOnlySystemParam for NonSendRef<'_, T> {}

unsafe impl<T: Resource> SystemParam for NonSendRef<'_, T> {
    type State = ResourceId;
    type Item<'world, 'state> = NonSendRef<'world, T>;
    // Because the resource is !Sync, we can only borrow it
    // on the main thread. In other words, this system is !Send.
    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
        // We do not prepare resource here,
        // thereby delaying memory allocation.
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_reading_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.read_only();
            if let Some(data) = world.storages.res_set.get(*state)
                && let Some(ptr) = data.get_ref(last_run, this_run)
            {
                Ok(ptr.into_non_send::<T>())
            } else {
                Err(uninit_resource_error::<Self, T>())
            }
        }
    }
}

// -----------------------------------------------------------------------------
// NonSendMut

unsafe impl<T: Resource> SystemParam for NonSendMut<'_, T> {
    type State = ResourceId;
    type Item<'world, 'state> = NonSendMut<'world, T>;
    // Because the resource is !Sync, we can only borrow it
    // on the main thread. In other words, this system is !Send.
    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_writing_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.data_mut();
            if let Some(data) = world.storages.res_set.get_mut(*state)
                && let Some(ptr) = data.get_mut(last_run, this_run)
            {
                Ok(ptr.into_non_send::<T>())
            } else {
                Err(uninit_resource_error::<Self, T>())
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Option<NonSend>

unsafe impl<T: Resource> ReadOnlySystemParam for Option<NonSend<'_, T>> {}

unsafe impl<T: Resource> SystemParam for Option<NonSend<'_, T>> {
    type State = ResourceId;
    type Item<'world, 'state> = Option<NonSend<'world, T>>;
    // Because the resource is !Sync, we can only borrow it
    // on the main thread. In other words, this system is !Send.
    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_reading_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.read_only();
            let Some(data) = world.storages.res_set.get(*state) else {
                return Ok(None);
            };
            let Some(ptr) = data.get_data() else {
                return Ok(None);
            };
            ptr.debug_assert_aligned::<T>();
            Ok(Some(NonSend {
                value: ptr.deref::<T>(),
            }))
        }
    }
}

// -----------------------------------------------------------------------------
// Option<NonSendRef>

unsafe impl<T: Resource> ReadOnlySystemParam for Option<NonSendRef<'_, T>> {}

unsafe impl<T: Resource> SystemParam for Option<NonSendRef<'_, T>> {
    type State = ResourceId;
    type Item<'world, 'state> = Option<NonSendRef<'world, T>>;
    // Because the resource is !Sync, we can only borrow it
    // on the main thread. In other words, this system is !Send.
    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_reading_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.read_only();
            let Some(data) = world.storages.res_set.get(*state) else {
                return Ok(None);
            };
            let Some(untyped) = data.get_ref(last_run, this_run) else {
                return Ok(None);
            };
            Ok(Some(untyped.into_non_send::<T>()))
        }
    }
}

// -----------------------------------------------------------------------------
// Option<NonSendMut>

unsafe impl<T: Resource> SystemParam for Option<NonSendMut<'_, T>> {
    type State = ResourceId;
    type Item<'world, 'state> = Option<NonSendMut<'world, T>>;
    // Because the resource is !Sync, we can only borrow it
    // on the main thread. In other words, this system is !Send.
    const NON_SEND: bool = true;
    const EXCLUSIVE: bool = false;

    fn init_state(world: &mut World) -> Self::State {
        world.register_resource::<T>()
    }

    fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
        table.set_writing_res(*state)
    }

    unsafe fn build_param<'w, 's>(
        world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, SystemParamError> {
        unsafe {
            let world = world.data_mut();
            let Some(data) = world.storages.res_set.get_mut(*state) else {
                return Ok(None);
            };
            let Some(untyped) = data.get_mut(last_run, this_run) else {
                return Ok(None);
            };
            Ok(Some(untyped.into_non_send::<T>()))
        }
    }
}
