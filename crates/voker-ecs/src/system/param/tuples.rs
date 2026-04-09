use super::{ReadOnlySystemParam, SystemParam};
use crate::error::GameError;
use crate::system::{AccessTable, SystemMeta};
use crate::tick::Tick;
use crate::world::{DeferredWorld, UnsafeWorld, World};

macro_rules! impl_tuple {
    (0: []) => {
        unsafe impl ReadOnlySystemParam for () {}

        unsafe impl SystemParam for () {
            type State = ();
            type Item<'world, 'state> = ();

            const NON_SEND: bool = false;
            const EXCLUSIVE: bool = false;

            fn init_state(_world: &mut World) -> Self::State {}

            fn mark_access(_table: &mut AccessTable, _state: &Self::State) -> bool { true }

            unsafe fn build_param<'w, 's>(
                _world: UnsafeWorld<'w>,
                _state: &'s mut Self::State,
                _last_run: Tick,
                _this_run: Tick,
            ) -> Result<Self::Item<'w, 's>, GameError> {
                Ok(())
            }
        }
    };
    (1 : [ $index:tt : $name:ident ]) => {
        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: ReadOnlySystemParam> ReadOnlySystemParam for ($name,) {}

        #[cfg_attr(docsrs, doc(fake_variadic))]
        #[cfg_attr(docsrs, doc = "This trait is implemented for tuples up to 12 items long.")]
        unsafe impl<$name: SystemParam> SystemParam for ($name,) {
            type State = <$name>::State;
            type Item<'world, 'state> = ( <$name>::Item<'world, 'state>, );

            const DEFERRED: bool = <$name>::DEFERRED;
            const NON_SEND: bool = <$name>::NON_SEND;
            const EXCLUSIVE: bool = <$name>::EXCLUSIVE;

            #[inline]
            fn init_state(world: &mut World) -> Self::State {
                <$name>::init_state(world)
            }

            #[inline]
            fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
                <$name>::mark_access(table, state)
            }

            #[inline]
            unsafe fn build_param<'w, 's>(
                world: UnsafeWorld<'w>,
                state: &'s mut Self::State,
                last_run: Tick,
                this_run: Tick,
            ) -> Result<Self::Item<'w, 's>, GameError> {
                unsafe { Ok(( <$name>::build_param(world, state, last_run, this_run)?, )) }
            }

            #[inline]
            fn defer(state: &mut Self::State, system_meta: &SystemMeta, world: DeferredWorld) {
                <$name>::defer(state, system_meta, world);
            }

            #[inline]
            fn apply_deferred(state: &mut Self::State, system_meta: &SystemMeta, world: &mut World) {
                <$name>::apply_deferred(state, system_meta, world);
            }
        }
    };
    ($num:literal : [$($index:tt : $name:ident),*]) => {
        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: ReadOnlySystemParam),*> ReadOnlySystemParam for ($($name),*) {}

        #[cfg_attr(docsrs, doc(hidden))]
        unsafe impl<$($name: SystemParam),*> SystemParam for ($($name),*) {
            type State = ( $( <$name>::State ),* );
            type Item<'world, 'state> = ( $( <$name>::Item<'world, 'state> ),* );

            const DEFERRED: bool = { false $( || <$name>::DEFERRED )* };
            const NON_SEND: bool = { false $( || <$name>::NON_SEND )* };
            const EXCLUSIVE: bool = { false $( || <$name>::EXCLUSIVE )* };

            fn init_state(world: &mut World) -> Self::State {
                ( $( <$name>::init_state(world) ),* )
            }

            fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
                true $( && <$name>::mark_access(table, &state.$index) )*
            }

            unsafe fn build_param<'w, 's>(
                world: UnsafeWorld<'w>,
                state: &'s mut Self::State,
                last_run: Tick,
                this_run: Tick,
            ) -> Result<Self::Item<'w, 's>, GameError> {
                unsafe { Ok(( $( <$name>::build_param(world, &mut state.$index, last_run, this_run)? ),* )) }
            }

            #[inline]
            fn defer(state: &mut Self::State, system_meta: &SystemMeta, mut world: DeferredWorld) {
                $( <$name>::defer(&mut state.$index, system_meta, world.reborrow()); )*
            }

            #[inline]
            fn apply_deferred(state: &mut Self::State, system_meta: &SystemMeta, world: &mut World) {
                $( <$name>::apply_deferred(&mut state.$index, system_meta, world); )*
            }
        }
    };
}

voker_utils::range_invoke!(impl_tuple, 12);
