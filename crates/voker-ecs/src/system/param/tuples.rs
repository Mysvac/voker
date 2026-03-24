use super::{ReadOnlySystemParam, SystemParam};
use crate::error::EcsError;
use crate::system::AccessTable;
use crate::tick::Tick;
use crate::world::{UnsafeWorld, World};

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
            ) -> Result<Self::Item<'w, 's>, EcsError> {
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

            const NON_SEND: bool = <$name>::NON_SEND;
            const EXCLUSIVE: bool = <$name>::EXCLUSIVE;

            fn init_state(world: &mut World) -> Self::State {
                <$name>::init_state(world)
            }

            fn mark_access(table: &mut AccessTable, state: &Self::State) -> bool {
                <$name>::mark_access(table, state)
            }

            unsafe fn build_param<'w, 's>(
                world: UnsafeWorld<'w>,
                state: &'s mut Self::State,
                last_run: Tick,
                this_run: Tick,
            ) -> Result<Self::Item<'w, 's>, EcsError> {
                unsafe { Ok(( <$name>::build_param(world, state, last_run, this_run)?, )) }
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
            ) -> Result<Self::Item<'w, 's>, EcsError> {
                unsafe { Ok(( $( <$name>::build_param(world, &mut state.$index, last_run, this_run)? ),* )) }
            }
        }
    };
}

voker_utils::range_invoke!(impl_tuple, 12);
