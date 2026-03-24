use core::ops::{Deref, DerefMut};

use super::{ReadOnlySystemParam, SystemParam};
use crate::error::EcsError;
use crate::system::AccessTable;
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

/// A system-local variable.
///
/// When used as a system parameter, each compiled system instance owns one
/// independent value of `T`. This makes `Local<T>` a convenient alternative to
/// global `static` state for per-system counters, caches, and temporary state.
///
/// The value is initialized from `T::default()` during system initialization
/// and then reused across subsequent runs of that system.
///
/// # Examples
///
/// ```ignore
/// fn system(mut counter: Local<u64>) {
///     *counter += 1;
/// }
/// ```
#[derive(Debug)]
pub struct Local<'s, T: Default + Send + Sync + 'static>(&'s mut T);

impl<'s, T: Default + Send + Sync + 'static> Deref for Local<'s, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'s, T: Default + Send + Sync + 'static> DerefMut for Local<'s, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

unsafe impl<T: Default + Send + Sync + 'static> ReadOnlySystemParam for Local<'_, T> {}

unsafe impl<T: Default + Send + Sync + 'static> SystemParam for Local<'_, T> {
    type State = T;
    type Item<'world, 'state> = Local<'state, T>;

    const NON_SEND: bool = false;
    const EXCLUSIVE: bool = false;

    fn init_state(_world: &mut World) -> Self::State {
        T::default()
    }

    fn mark_access(_table: &mut AccessTable, _state: &Self::State) -> bool {
        true
    }

    unsafe fn build_param<'w, 's>(
        _world: UnsafeWorld<'w>,
        state: &'s mut Self::State,
        _last_run: Tick,
        _this_run: Tick,
    ) -> Result<Self::Item<'w, 's>, EcsError> {
        Ok(Local(state))
    }
}
